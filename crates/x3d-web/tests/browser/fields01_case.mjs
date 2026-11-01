// The field API 01 cases (issue #137): the field classes of the X3D namespace, X3DConstants, live SF and MF field
// objects of nodes, assignments of every kind of field, X3DField.addFieldCallback and X3DNode.getFieldDefinitions.
// oracle/x_ite-web-runtime-02.mjs (--suite fields01) runs them against pinned X_ITE 16.5.0 and records the values;
// fields01.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
//
// The events of a field the page changes are processed in the next frame, in both engines; each case waits frames
// (`settle`) before it reads what they did.

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Scripting:1
DEF T Transform {
   children DEF S Shape {
      appearance DEF A Appearance { material DEF M Material { diffuseColor 0.2 0.4 0.9 } }
      geometry IndexedFaceSet { coordIndex [ 0 1 2 -1 ] coord DEF C Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } }
   }
}
DEF T2 Transform { }
DEF T3 Transform { }
DEF M2 Material { }
DEF C2 Coordinate { }
DEF A2 Appearance { }
DEF G Group { }
DEF P PixelTexture { }
DEF I IndexedLineSet { }
DEF V Viewpoint { description "v" }
DEF E Script { field SFInt32 n 5 field SFVec3f v 1 2 3 inputOnly SFBool go outputOnly SFString out }
ROUTE M.diffuseColor_changed TO M2.set_diffuseColor
ROUTE C.point_changed TO C2.set_point
ROUTE T.translation_changed TO T2.set_translation
ROUTE T.translation_changed TO T3.set_translation
`)}`;

// Load the world; resolve with "resolved" or the error.
async function load (page)
{
   await page .evaluate (pageLoadURL, [ WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);
}

// The name, type and text of a field value as both engines give it.
function pageDescribe ()
{
   window .describe = value =>
   {
      if (value === null || value === undefined || typeof value !== "object")
         return { type: typeof value, value: value === undefined ? "undefined" : value };

      return { type: value .getTypeName ?.() ?? Object .prototype .toString .call (value), value: String (value) };
   };
}

// The X3D namespace: its field classes and their text, X3DConstants.
async function namespace (page)
{
   return page .evaluate (() =>
   {
      const X3D     = window .X3D_NS;
      const classes = Object .keys (X3D) .filter (name => /^(SF|MF)/ .test (name)) .sort ();

      return {
         classes,
         others: [ "X3DField", "X3DArrayField", "X3DFieldDefinition", "FieldDefinitionArray", "X3DConstants" ] .map (name => [ name, typeof X3D [name] ]),
         defaults: classes .filter (name => name !== "SFNode") .map (name => { const f = new X3D [name] (); return [ name, f .getTypeName (), String (f), f instanceof X3D .X3DField, f instanceof X3D .X3DArrayField ]; }),
         constants: Object .entries (X3D .X3DConstants) .filter (([ name ]) => !/^X3D|^[A-Z][a-z]/ .test (name) || /^X3D(Node|Field|ChildNode|Bounded)/ .test (name)),
         constantCount: Object .keys (X3D .X3DConstants) .length,
         access: [ "initializeOnly", "inputOnly", "outputOnly", "inputOutput" ] .map (name => X3D .X3DConstants [name]),
         events: [ "CONNECTION_ERROR", "BROWSER_EVENT", "INITIALIZED_EVENT", "SHUTDOWN_EVENT", "INITIALIZED_ERROR" ] .map (name => X3D .X3DConstants [name]),
         nodeTypes: [ "X3DNode", "Shape", "Transform", "Material", "X3DChildNode" ] .map (name => X3D .X3DConstants [name] !== undefined),
      };
   });
}

// The field classes as values: constructors, components, methods. Pure X_ITE code in both engines; the case shows
// the same classes are there.
async function values (page)
{
   return page .evaluate (() =>
   {
      const X3D = window .X3D_NS;
      const out = { };

      const c = new X3D .SFColor (1, 0.5, 0);
      out .color = [ String (c), c .r, c .g, c .b, String (c .getHSV ()) ];

      const v = new X3D .SFVec3f (1, 2, 3), w = new X3D .SFVec3f (4, 5, 6);
      out .vec3 = [ String (v .add (w)), String (v .cross (w)), v .dot (w), v .length (), String (v .normalize ()), String (v .multiply (2)) ];

      const r = new X3D .SFRotation (0, 1, 0, Math .PI / 2);
      out .rotation = [ String (r), String (r .multVec (new X3D .SFVec3f (1, 0, 0))), String (r .inverse ()), String (r .slerp (new X3D .SFRotation (), 0.5)) ];

      const m = new X3D .SFMatrix4f ();
      m .setTransform (new X3D .SFVec3f (1, 2, 3), r, new X3D .SFVec3f (2, 2, 2));
      out .matrix = [ String (m), String (m .inverse ()), String (m .multVecMatrix (new X3D .SFVec3f (1, 1, 1))) ];

      const image = new X3D .SFImage (2, 1, 3, new X3D .MFInt32 (0xff0000, 0x00ff00));
      out .image = [ String (image), image .width, image .height, image .comp, Array .from (image .array) ];

      const mf = new X3D .MFVec3f (v, w);
      mf .push (new X3D .SFVec3f (7, 8, 9));
      mf [0] .x = 10;
      out .mfvec3 = [ String (mf), mf .length, String (mf [2]) ];

      const strings = new X3D .MFString ("a", "b \"c\"");
      out .mfstring = [ String (strings), strings .length, strings [1] ];

      const floats = new X3D .MFFloat (1, 2, 3);
      floats .length = 2;
      out .mffloat = [ String (floats), floats .length ];

      out .quaternion = String (new X3D .SFQuaternion (0, 0, 0, 1));
      out .vec2d = String (new X3D .SFVec2d (1.5, -2));
      out .vec4f = String (new X3D .SFVec4f (1, 2, 3, 4));
      out .matrix3 = String (new X3D .SFMatrix3d ());
      out .equals = [ v .equals (new X3D .SFVec3f (1, 2, 3)), c .equals (new X3D .SFColor (1, 0.5, 0)) ];
      out .copy = String (r .copy ());

      return out;
   });
}

// Field objects of nodes are live: the same object each read, a changed component or element reaches the node (and
// its routes), and the engine's changes reach the object.
async function live (page)
{
   await load (page);

   const before = await page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const M = scene .getNamedNode ("M"), C = scene .getNamedNode ("C");

      window .held = { color: M .diffuseColor, point: C .point, M2: scene .getNamedNode ("M2"), C2: scene .getNamedNode ("C2") };

      const out = {
         same: M .diffuseColor === M .diffuseColor,
         sameMF: C .point === C .point,
         colorType: M .diffuseColor .getTypeName (),
         pointType: C .point .getTypeName (),
         pointLength: C .point .length,
         point1: String (C .point [1]),
      };

      M .diffuseColor .r = 1;
      C .point [1] .x = 5;
      C .point .push (new window .X3D_NS .SFVec3f (7, 8, 9));

      out .afterSet = [ String (M .diffuseColor), String (C .point), C .point .length ];
      return out;
   });

   await settle (page, 100);

   const after = await page .evaluate (() =>
   {
      const { color, point, M2, C2 } = window .held;

      return {
         held: [ String (color), String (point) ],
         routed: [ String (M2 .diffuseColor), String (C2 .point), C2 .point .length ],
      };
   });

   // The engine changes the routed field: an object read before keeps up.
   const routed = await page .evaluate (async () =>
   {
      const scene  = document .querySelector ("x3d-canvas") .browser .currentScene;
      const T = scene .getNamedNode ("T"), T2 = scene .getNamedNode ("T2");
      const held   = T2 .translation;

      T .translation = new window .X3D_NS .SFVec3f (1, 2, 3);

      return { now: String (held) };
   });

   await settle (page, 100);

   routed .later = await page .evaluate (() => String (document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("T2") .translation));

   return { before, after, routed };
}

// Assignments through the SFNode proxy, for each kind of field, read after the next frame.
async function assign (page)
{
   await load (page);

   const result = await page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const node  = name => scene .getNamedNode (name);
      const out   = { };

      const tryAssign = (name, field, value) =>
      {
         try
         {
            out [name] = Reflect .set (node (name .split (".") [0]), field, value);
         }
         catch (error)
         {
            out [name] = `${error .name}: ${error .message}`;
         }
      };

      tryAssign ("T.translation", "translation", new X3D .SFVec3f (1, 2, 3));
      tryAssign ("T.rotation", "rotation", new X3D .SFRotation (0, 0, 1, 1));
      tryAssign ("T.scale", "scale", new X3D .SFVec3f (2, 2, 2));
      tryAssign ("M.transparency", "transparency", 0.25);
      tryAssign ("M.emissiveColor", "emissiveColor", new X3D .SFColor (0.1, 0.2, 0.3));
      tryAssign ("V.description", "description", "w \"x\"");
      tryAssign ("V.jump", "jump", false);
      tryAssign ("I.coordIndex", "coordIndex", new X3D .MFInt32 (0, 1, -1));
      tryAssign ("C2.point", "point", new X3D .MFVec3f (new X3D .SFVec3f (1, 1, 1)));
      tryAssign ("S.appearance", "appearance", node ("A2"));
      tryAssign ("G.children", "children", new X3D .MFNode (node ("T2"), node ("T3")));
      tryAssign ("P.image", "image", new X3D .SFImage (1, 2, 3, new X3D .MFInt32 (0xff0000, 0x0000ff)));
      tryAssign ("E.n", "n", 9);
      tryAssign ("E.v", "v", new X3D .SFVec3f (4, 5, 6));
      // outputOnly: the proxy refuses it.
      tryAssign ("E.out", "out", "z");
      tryAssign ("T.bboxCenter", "bboxCenter", new X3D .SFVec3f (1, 1, 1));

      return out;
   });

   await settle (page, 100);

   const read = await page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const node  = name => scene .getNamedNode (name);

      return {
         T: [ String (node ("T") .translation), String (node ("T") .rotation), String (node ("T") .scale), String (node ("T") .bboxCenter) ],
         M: [ node ("M") .transparency, String (node ("M") .emissiveColor) ],
         V: [ node ("V") .description, node ("V") .jump ],
         I: String (node ("I") .coordIndex),
         C2: String (node ("C2") .point),
         S: node ("S") .appearance .getNodeName (),
         G: Array .from (node ("G") .children, n => n .getNodeName ()),
         P: String (node ("P") .image),
         E: [ node ("E") .n, String (node ("E") .v), node ("E") .out ],
         routed: String (node ("T2") .translation),
      };
   });

   return { result, read };
}

// X3DField.addFieldCallback: the callbacks of each event, in event order, with the value of the event.
async function callbacks (page)
{
   await load (page);

   await page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const T = scene .getNamedNode ("T"), T2 = scene .getNamedNode ("T2"), T3 = scene .getNamedNode ("T3");

      window .calls = [ ];

      const record = name => value => window .calls .push ([ name, String (value), value ?.getTypeName ?.() ]);

      for (const [ name, node ] of [ [ "T", T ], [ "T2", T2 ], [ "T3", T3 ] ])
      {
         node .getField ("translation") .addFieldCallback ("a", record (`${name}.a`));
         node .getField ("translation") .addFieldCallback ("b", record (`${name}.b`));
      }

      window .counts = [ T .getField ("translation") .getFieldCallbacks () .size ];

      T .translation = new window .X3D_NS .SFVec3f (1, 2, 3);
   });

   await settle (page, 100);

   await page .evaluate (() =>
   {
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const T = scene .getNamedNode ("T"), T3 = scene .getNamedNode ("T3");

      window .calls .push ([ "frame" ]);
      T3 .getField ("translation") .removeFieldCallback ("a");
      window .counts .push (T3 .getField ("translation") .getFieldCallbacks () .size);
      T .translation .x = 4;
   });

   await settle (page, 100);

   return page .evaluate (() => ({ calls: window .calls, counts: window .counts }));
}

// X3DNode.getFieldDefinitions and the SFNode proxy's keys.
async function definitions (page)
{
   await load (page);

   return page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const list  = node => Array .from (node .getFieldDefinitions (), d => [ d .name, d .accessType, d .dataType, String (d .value) ]);
      const M     = scene .getNamedNode ("M");
      const E     = scene .getNamedNode ("E");
      const definitions = M .getFieldDefinitions ();

      return {
         material: list (M),
         script: list (E) .filter (([ name ]) => [ "n", "v", "go", "out" ] .includes (name)),
         isArray: definitions instanceof X3D .FieldDefinitionArray,
         get: [ definitions .get ("diffuseColor") instanceof X3D .X3DFieldDefinition, String (definitions .get ("diffuseColor") .value), String (definitions .get ("nothing")) ],
         keys: Object .keys (M) .slice (0, 6),
         has: [ "diffuseColor" in M, "nothing" in M ],
         getField: [ M .getField ("diffuseColor") .getName (), M .getField ("diffuseColor") .getAccessType (), M .getField ("diffuseColor_changed") .getName () ],
         unknown: (() => { try { M .getField ("nothing"); return "no error"; } catch (error) { return error .message; } }) (),
      };
   });
}

// X3DScene.rootNodes, SFNode and MFNode values.
async function nodes (page)
{
   await load (page);

   return page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const roots = scene .rootNodes;
      const T     = scene .getNamedNode ("T");

      return {
         roots: [ roots instanceof X3D .MFNode, roots .getTypeName (), roots .length, Array .from (roots, n => n .getNodeName ()) ],
         children: [ T .children instanceof X3D .MFNode, T .children .length, T .children [0] .getNodeTypeName () ],
         sfnode: [ T .children [0] .appearance instanceof X3D .SFNode, T .children [0] .appearance .getNodeName (), T .children [0] .appearance .material .diffuseColor .getTypeName () ],
         same: scene .getNamedNode ("M") === T .children [0] .appearance .material,
         nodeType: Array .from (T .getNodeType ()) .includes (X3D .X3DConstants .Transform),
         typeName: [ T .getTypeName (), T .getNodeTypeName (), T .getNodeName () ],
      };
   });
}

const cases = { namespace, values, live, assign, callbacks, definitions, nodes };

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageDescribe); return run (page, env); };


// ---- Round trip of every field type through the SAI (issue #137) -----------------------------------------------------------
//
// A world with two Scripts, E and F, that have a field of every SF and MF type except SFNode and MFNode (those are the
// subject of fields03 and rootnodes01). Each field of E is routed to the same field of F. Each step the page changes the
// field of E in one way (an assignment, a changed component or element, setValue, push, splice, length) and, after the frames
// that carry the events, the case reads what the page's field object holds (`s`: its text, `p`: its components), what the
// world holds (`t`: the text form of the node, which the engine writes), what the routed field of F holds (`f`) and whether
// the object read before is still the field's object (`same`).

const SF_TYPES = [ "SFBool", "SFColor", "SFColorRGBA", "SFDouble", "SFFloat", "SFImage", "SFInt32", "SFMatrix3d", "SFMatrix3f",
   "SFMatrix4d", "SFMatrix4f", "SFRotation", "SFString", "SFTime", "SFVec2d", "SFVec2f", "SFVec3d", "SFVec3f", "SFVec4d", "SFVec4f" ];
const MF_TYPES = SF_TYPES .map (type => type .replace (/^SF/, "MF"));

// The default values the Script fields are declared with.
const DEFAULTS = { SFBool: "FALSE", SFColor: "0 0 0", SFColorRGBA: "0 0 0 0", SFDouble: "0", SFFloat: "0", SFImage: "0 0 0", SFInt32: "0",
   SFMatrix3d: "1 0 0 0 1 0 0 0 1", SFMatrix3f: "1 0 0 0 1 0 0 0 1", SFMatrix4d: "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1", SFMatrix4f: "1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1",
   SFRotation: "0 0 1 0", SFString: "\"\"", SFTime: "0", SFVec2d: "0 0", SFVec2f: "0 0", SFVec3d: "0 0 0", SFVec3f: "0 0 0", SFVec4d: "0 0 0 0", SFVec4f: "0 0 0 0" };

const fieldDeclarations = [ ... SF_TYPES .map (type => `inputOutput ${type} f${type} ${DEFAULTS [type]}`), ... MF_TYPES .map (type => `inputOutput ${type} f${type} [ ]`) ] .join (" ");

const ROUND_TRIP_WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Scripting:1
DEF E Script { ${fieldDeclarations} }
DEF F Script { ${fieldDeclarations} }
${[ ... SF_TYPES, ... MF_TYPES ] .map (type => `ROUTE E.f${type}_changed TO F.set_f${type}`) .join ("\n")}
`)}`;

async function roundTripLoad (page)
{
   await page .evaluate (pageLoadURL, [ ROUND_TRIP_WORLD ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);
}

// The page's helpers: `sample (type, k)` is a value of the SF or MF type, k = 0, 1, 2 (distinct, the first not exact in
// float32), `plain` the components of a value, `lineOf` the text of one field in a node's text form.
function pageRoundTripHelpers ()
{
   const X3D = window .X3D_NS;

   const scalars = { Double: [ 0.1, 1.5, -22500000000 ], Float: [ 0.1, 1.5, -22500000000 ], Time: [ 0.1, 1000000000.125, 0 ], Int32: [ 7, -9, 2147483647 ],
      Bool: [ true, false, true ], String: [ "a b", "q\"x\\y\nzé☃", "" ] };

   // The numbers of vector, matrix and color samples: 0.1 * (i + 1) the first time, then multiples of 0.25.
   const num = (k, i, count) => k === 0 ? (i + 1) / 10 : k === 1 ? 10 + (i + 1) * 0.25 : -(i + 1) / 4 + (i === count - 1 ? 0.5 : 0);
   const nums = (k, count) => Array .from ({ length: count }, (_, i) => num (k, i, count));
   const colors = [ [ 0.1, 0.2, 0.3, 0.4 ], [ 0.25, 0.5, 0.75, 1 ], [ 1, 0.5, 0, 0.125 ] ];
   const images = [ [ 2, 2, 3, [ 0x112233, 0x445566, 0x778899, 0xaabbcc ] ], [ 1, 2, 4, [ 0x11223344, 0x55667788 ] ], [ 0, 0, 0, [ ] ] ];
   const rotations = [ [ 0, 0, 1, 0.1 ], [ 0, 3, 0, 1.5 ], [ 1, 0, 0, -2.5 ] ];

   const sf = (type, k) =>
   {
      const name = type .replace (/^SF/, "");

      if (name in scalars)
         return scalars [name] [k];

      switch (name)
      {
         case "Color":     return new X3D .SFColor (... colors [k] .slice (0, 3));
         case "ColorRGBA": return new X3D .SFColorRGBA (... colors [k]);
         case "Image":     return new X3D .SFImage (images [k] [0], images [k] [1], images [k] [2], new X3D .MFInt32 (... images [k] [3]));
         case "Rotation":  return new X3D .SFRotation (... rotations [k]);
         case "Matrix3d": case "Matrix3f": return new X3D [type] (... nums (k, 9));
         case "Matrix4d": case "Matrix4f": return new X3D [type] (... nums (k, 16));
         default:          return new X3D [type] (... nums (k, Number (name .at (-2))));
      }
   };

   // An MF sample has 2, 3 or 0 elements; the elements of an MF of scalars are scalars.
   window .sample = (type, k) =>
   {
      if (type .startsWith ("SF"))
         return sf (type, k);

      const count = [ 2, 3, 0 ] [k];

      return new X3D [type] (... Array .from ({ length: count }, (_, j) => sf (type .replace (/^MF/, "SF"), (k + j) % 3)));
   };

   window .element = (type, k) => sf (type .replace (/^MF/, "SF"), k);

   const plain = value =>
   {
      if (value === null || value === undefined || typeof value !== "object")
         return value === undefined ? "undefined" : value;

      if (ArrayBuffer .isView (value))
         return Array .from (value);

      if (value instanceof X3D .X3DArrayField)
         return Array .from (value, plain);

      if (value instanceof X3D .SFImage)
         return [ value .width, value .height, value .comp, Array .from (value .array) ];

      if (typeof value .getValue === "function")
         return plain (value .getValue ());

      return String (value);
   };

   window .plain = plain;

   window .lineOf = (node, name) =>
   {
      const text  = node .toVRMLString ();
      const start = text .indexOf (` ${name} `);

      if (start < 0)
         return "missing";

      const rest = text .slice (start);
      const end  = rest .search (/\n\s*(?:inputOutput|initializeOnly|field|inputOnly|outputOnly|\})/);

      return (end < 0 ? rest : rest .slice (0, end)) .replace (/\s+/g, " ");
   };

   window .attempt = fn =>
   {
      try
      {
         return fn ();
      }
      catch (error)
      {
         return `${error ?.constructor ?.name}: ${error ?.message}`;
      }
   };
}

// What the page and the world hold for each type, after the frames of a step.
function pageObserve (types)
{
   const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
   const E = scene .getNamedNode ("E"), F = scene .getNamedNode ("F");
   const out = { };

   for (const type of types)
   {
      const name = `f${type}`;

      out [type] = {
         s: window .attempt (() => String (E [name])),
         p: window .attempt (() => window .plain (E [name])),
         t: window .attempt (() => window .lineOf (E, name)),
         f: window .attempt (() => window .plain (F [name])),
         ft: window .attempt (() => window .lineOf (F, name)),
         same: window .held [type] === E [name],
      };
   }

   return out;
}

// One scenario: `steps` are the page's actions on every type of `types`, in order (`act` of pageStep).
async function roundTrip (page, types, steps)
{
   await roundTripLoad (page);
   await page .evaluate (pageRoundTripHelpers);

   await page .evaluate (types =>
   {
      const E = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("E");

      window .held = { };

      for (const type of types)
         window .held [type] = E [`f${type}`];
   }, types);

   const out = { initial: await page .evaluate (pageObserve, types) };

   for (const step of steps)
   {
      out [step] = { };

      out [step] .acted = await page .evaluate (pageStep, [ types, step ]);

      await settle (page, 100);

      out [step] .seen = await page .evaluate (pageObserve, types);
   }

   out .log = page .log .filter (line => !/^(log|info|debug):/ .test (line));

   return out;
}

// The action `step` on the field of every type; the result of each is its error, if any.
function pageStep ([ types, step ])
{
   const X3D = window .X3D_NS;
   const E   = document .querySelector ("x3d-canvas") .browser .currentScene .getNamedNode ("E");
   const out = { };

   for (const type of types)
   {
      const name = `f${type}`, mf = type .startsWith ("MF");

      const actions =
      {
         assign0: () => { E [name] = window .sample (type, 0); },
         assign1: () => { E [name] = window .sample (type, 1); },
         assign2: () => { E [name] = window .sample (type, 2); },
         setValue1: () => E .getField (name) .setValue (window .sample (type, 1)),
         // SF: a changed component or element of the live object.
         component: () =>
         {
            const value = E [name];

            switch (type)
            {
               case "SFColor": value .r = 0.9; break;
               case "SFColorRGBA": value .a = 0.9; break;
               case "SFRotation": value .angle = 2.25; break;
               case "SFImage": value .array [0] = 0x010203; break;
               case "SFMatrix3d": case "SFMatrix3f": case "SFMatrix4d": case "SFMatrix4f": value [1] = 7.5; break;
               case "SFVec2d": case "SFVec2f": case "SFVec3d": case "SFVec3f": case "SFVec4d": case "SFVec4f": value .y = 7.5; break;
               default: return "no component";
            }
         },
         // MF: a changed element, and the array's own changes.
         elementIndex: () => { E [name] [0] = window .element (type, 2); },
         elementComponent: () =>
         {
            const value = E [name] [0];

            if (value === undefined || value === null || typeof value !== "object")
               return "no component";

            switch (type)
            {
               case "MFColor": value .g = 0.8; break;
               case "MFColorRGBA": value .a = 0.8; break;
               case "MFRotation": value .angle = 2.25; break;
               case "MFImage": value .array [0] = 0x010203; break;
               case "MFMatrix3d": case "MFMatrix3f": case "MFMatrix4d": case "MFMatrix4f": value [1] = 7.5; break;
               case "MFVec2d": case "MFVec2f": case "MFVec3d": case "MFVec3f": case "MFVec4d": case "MFVec4f": value .y = 7.5; break;
               default: return "no component";
            }
         },
         push: () => { E [name] .push (window .element (type, 1)); },
         splice: () => { E [name] .splice (1, 1, window .element (type, 2), window .element (type, 0)); },
         unshift: () => { E [name] .unshift (window .element (type, 2)); },
         length1: () => { E [name] .length = 1; },
         length0: () => { E [name] .length = 0; },
         grow: () => { E [name] .length = 3; },
      };

      if (actions [step] === undefined)
         throw new Error (`unknown step ${step}`);

      if (!mf && [ "elementIndex", "elementComponent", "push", "splice", "unshift", "length1", "length0", "grow" ] .includes (step))
         continue;

      if (mf && step === "component")
         continue;

      const error = window .attempt (actions [step]);

      if (error !== undefined)
         out [type] = error;
   }

   return out;
}

const roundTripCases =
{
   // The SF types: an assignment, a changed component, setValue, and a second assignment.
   sf: async page => roundTrip (page, SF_TYPES, [ "assign0", "component", "setValue1", "assign2", "assign1" ]),

   // The MF types: an assignment, a changed element, the array's own changes, setValue and a second assignment.
   mf: async page => roundTrip (page, MF_TYPES, [ "assign0", "elementIndex", "elementComponent", "push", "splice", "unshift", "length1", "grow", "setValue1", "length0", "assign2", "assign1" ]),
};

for (const [ name, run ] of Object .entries (roundTripCases))
   roundTripCases [name] = async (page, env) => { await page .evaluate (pageDescribe); return run (page, env); };


// ---- X3DConstants: all node types at once, or as components load (issue #137) ------------------------------------------------

const CONSTANTS_FULL = `data:model/x3d-vrml,${encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Full\n")}`;
const CONSTANTS_NODES = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Full
COMPONENT Geospatial:1
COMPONENT HAnim:1
DEF G GeoOrigin { }
DEF T Text { string "a" fontStyle FontStyle { } }
DEF N NurbsCurve { }
DEF H HAnimHumanoid { }
DEF K KeySensor { }
`)}`;

async function loadWorld (page, url)
{
   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => window .pending);

   await settle (page, 100);

   return result;
}

// The constants of the X3DConstants object as [name, value] in its order.
const pageConstants = () => Object .entries (window .X3D_NS .X3DConstants);

// X3DConstants: before any world, after a world of the Full profile and after nodes of several components. The first
// recording is the whole list; each later one lists the names added since. X_ITE numbers the constants of a component as the
// component's module loads, and the modules of a profile load in parallel, so their values differ from run to run: only the
// names, and whether a name is defined, are recorded for the added ones.
async function constants (page)
{
   const first = await page .evaluate (pageConstants);
   const out   = { first: { count: first .length, entries: first } };
   let   known = new Set (first .map (([ name ]) => name));

   const probe = () => page .evaluate (() =>
   {
      const X3D = window .X3D_NS;

      return [ "Script", "Text", "FontStyle", "GeoOrigin", "GeoCoordinate", "NurbsCurve", "HAnimHumanoid", "KeySensor", "Geometry2D", "Arc2D", "SpecularGlossinessMaterial", "X3DNurbsControlCurveNode", "Transform", "X3DChildNode" ] .map (name => [ name, X3D .X3DConstants [name] !== undefined ]);
   });

   out .first .probe = await probe ();

   for (const [ key, url ] of [ [ "full", CONSTANTS_FULL ], [ "nodes", CONSTANTS_NODES ] ])
   {
      const result = await loadWorld (page, url);
      const now    = await page .evaluate (pageConstants);

      out [key] = { result, count: now .length, added: now .filter (([ name ]) => !known .has (name)) .map (([ name ]) => name) .sort (), probe: await probe () };
      known = new Set (now .map (([ name ]) => name));
   }

   // The constants of a node's getNodeType () against X3DConstants.
   out .nodeTypes = await page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;

      return [ "G", "T", "N", "H", "K" ] .map (name =>
      {
         const node = scene .getNamedNode (name);

         return [ name, node .getNodeTypeName (), Array .from (node .getNodeType ()) .map (type => Object .entries (X3D .X3DConstants) .find (([ , value ]) => value === type) ?.[0] ?? type) ];
      });
   });

   return out;
}

// ---- A call before the browser is ready (issue #137) -------------------------------------------------------------------------

// X_ITE starts at once, X_ITE_R's engine is a WebAssembly module and a WebGPU device that load asynchronously
// (browser .ready ()). A new browser (X3D.createBrowser ()) is used from the first line after it is made, attached or not;
// the same calls are made again after the browser is ready.
async function early (page, attached)
{
   await page .waitForFunction (() => window .ready || window .startError, null, { timeout: 30000 });

   const result = await page .evaluate (async attached =>
   {
      const X3D = window .X3D_NS;

      const attempt = fn =>
      {
         try
         {
            const value = fn ();

            if (value === undefined)
               return "undefined";

            if (value !== null && typeof value === "object")
               return value .getTypeName ?.() ?? value .constructor ?.name;

            return typeof value === "number" && !Number .isFinite (value) ? `number:${value}` : value;
         }
         catch (error)
         {
            return `${error ?.constructor ?.name}: ${error ?.message}`;
         }
      };

      const calls = browser =>
      {
         const out = { };
         let node, mf;

         out .scene = attempt (() => browser .currentScene);
         out .worldURL = attempt (() => { const url = browser .currentScene .worldURL; return url .startsWith ("data:") ? url : "page"; });
         out .rootNodes = attempt (() => browser .currentScene .rootNodes .length);
         out .createScene = attempt (() => browser .createScene ());
         out .createNode = attempt (() => { node = browser .currentScene .createNode ("Transform"); return node; });
         out .getName = attempt (() => node .getNodeName ());
         out .setField = attempt (() => { node .translation = new X3D .SFVec3f (1, 2, 3); return String (node .translation); });
         out .setMFField = attempt (() => { const g = browser .currentScene .createNode ("Group"); g .children = new X3D .MFNode (node); return g .children .length; });
         out .addRoot = attempt (() => { browser .currentScene .addRootNode (node); return browser .currentScene .rootNodes .length; });
         out .getNamed = attempt (() => browser .currentScene .getNamedNode ("T"));
         out .vrml = attempt (() => { mf = browser .createVrmlFromString ("Transform { translation 4 5 6 }"); return mf .length; });
         out .vrmlField = attempt (() => String (mf [0] .translation));
         out .fromString = attempt (() => String (X3D .SFVec3f .fromString ("1 2 3")));
         out .fromStringMF = attempt (() => String (X3D .MFVec3f .fromString ("[1 2 3, 4 5 6]")));
         out .fromStringNode = attempt (() => String (X3D .SFNode .fromString ("Transform { }")));
         out .classes = attempt (() => String (new X3D .SFMatrix4f () .multVecMatrix (new X3D .SFVec3f (1, 2, 3))));
         out .constants = attempt (() => X3D .X3DConstants .Transform);
         out .options = attempt (() => browser .getBrowserOption ("Shading"));
         out .setOption = attempt (() => browser .setBrowserOption ("Shading", "POINT"));
         out .routes = attempt (() => browser .currentScene .getRoutes () .length);
         out .toVRML = attempt (() => browser .currentScene .toVRMLString () .length > 0);
         out .loadURL = attempt (() => browser .loadURL (new X3D .MFString ("data:model/x3d-vrml,%23X3D%20V4.0%20utf8%0APROFILE%20Interchange%0ADEF%20Q%20Transform%20%7B%20%7D%0A")) .constructor .name);

         return out;
      };

      const element = X3D .createBrowser ();

      if (attached)
         document .body .append (element);

      const before = calls (element .browser);

      await new Promise (resolve => setTimeout (resolve, 100));

      // X_ITE_R only: wait for the engine (X_ITE has no such call and is ready at once).
      await element .browser .ready ?.();

      document .body .append (element);
      await new Promise (resolve => setTimeout (resolve, 800));

      return { before, after: calls (element .browser), unhandled: window .unhandled };
   }, attached);

   result .log = page .log .filter (line => !/^(log|info|debug):/ .test (line));

   return result;
}

const earlyCases =
{
   detached: async page => { await page .evaluate (() => { window .unhandled = [ ]; window .addEventListener ("unhandledrejection", event => { event .preventDefault (); window .unhandled .push (`${event .reason ?.constructor ?.name}: ${event .reason ?.message}`); }); }); return early (page, false); },
   attached: async page => { await page .evaluate (() => { window .unhandled = [ ]; window .addEventListener ("unhandledrejection", event => { event .preventDefault (); window .unhandled .push (`${event .reason ?.constructor ?.name}: ${event .reason ?.message}`); }); }); return early (page, true); },
};


// X3DExecutionContext.createNode (typeName): a new node made in the scene, not part of it until it is added.
async function createNodeCase (page, env, world = CONSTANTS_FULL)
{
   await page .evaluate (() => { window .attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : v; } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } }; });

   await loadWorld (page, world);

   const out = await page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const out   = { };
      const node  = type => scene .createNode (type);

      for (const type of [ "Transform", "Shape", "Box", "Material", "Group", "WorldInfo", "Script", "Text", "GeoOrigin", "X3DNode", "Nope", "", "transform", "Transform { }", "Transform Group" ])
      {
         out [`type ${type}`] = window .attempt (() =>
         {
            const n = node (type);

            return [ n .getTypeName (), n .getNodeTypeName (), n .getNodeName (), n .getNodeUserData ?.("x") ?? null, Array .from (n .getFieldDefinitions (), d => d .name) .length, n .toVRMLString () ];
         });
      }

      out .noArgument = window .attempt (() => scene .createNode ());
      out .numberArgument = window .attempt (() => scene .createNode (5) .getNodeTypeName ());
      out .nullArgument = window .attempt (() => scene .createNode (null) .getNodeTypeName ());

      const T = scene .createNode ("Transform");

      out .distinct = window .attempt (() => scene .createNode ("Transform") !== T);
      out .roots = scene .rootNodes .length;
      out .set = window .attempt (() => { T .translation = new X3D .SFVec3f (1, 2, 3); return String (T .translation); });
      out .rootsAfterSet = scene .rootNodes .length;

      out .add = window .attempt (() => { scene .addRootNode (T); return [ scene .rootNodes .length, scene .rootNodes [0] .getNodeTypeName () ]; });

      const child = scene .createNode ("Shape");
      const group = scene .createNode ("Group");

      group .children = new X3D .MFNode (child);
      out .child = window .attempt (() => [ group .children .length, group .children [0] .getNodeTypeName () ]);
      out .rootsAfterChild = scene .rootNodes .length;
      out .text = window .attempt (() => group .toVRMLString ());

      out .scene = window .attempt (() => scene .toVRMLString ());

      return out;
   });

   out .log = page .log .filter (line => !/^(log|info|debug):/ .test (line));

   return out;
}

export const CASE_GROUPS = {
   fieldAPI: { context: DESKTOP, cases },
   fieldRoundTrip: { context: DESKTOP, cases: roundTripCases },
   fieldConstants: { context: DESKTOP, cases: { constants: async (page, env) => { await page .waitForFunction (() => window .ready, null, { timeout: 30000 }); return constants (page, env); } } },
   fieldEarly: { context: DESKTOP, cases: { ... earlyCases, createNode: createNodeCase,
      // Nodes of external components the world did not load: a world of the Interchange profile (one of them first, then the same list).
      createNodeInterchange: (page, env) => createNodeCase (page, env, `data:model/x3d-vrml,${encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Interchange\n")}`) } },
};
