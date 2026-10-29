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

export const CASE_GROUPS = {
   fieldAPI: { context: DESKTOP, cases },
};
