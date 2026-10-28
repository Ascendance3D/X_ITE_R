// The field API 02 cases (issue #137): the text of nodes (toString, toVRMLString, toXMLString, toJSONString of SFNode and
// MFNode values) and values read from text (X3DField.fromString, fromVRMLString, fromXMLString).
// oracle/x_ite-web-runtime-02.mjs (--suite fields02) runs them against pinned X_ITE 16.5.0 and records the values;
// fields02.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Interchange
DEF T Transform {
   translation 1 2 3
   children [
      DEF S Shape {
         appearance Appearance { material DEF M Material { diffuseColor 0.2 0.4 0.9 } }
         geometry Box { size 2 3 4 }
      }
      USE S
   ]
}
DEF G Group { }
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

// The result of `fn`, or its error as `Name: message`; the same text in both engines.
function pageAttempt ()
{
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

// toString and the text forms of an SFNode, an MFNode and NULL.
async function text (page)
{
   await load (page);

   return page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const T     = scene .getNamedNode ("T");
      const M     = scene .getNamedNode ("M");
      const forms = value => [ "toString", "toVRMLString", "toXMLString", "toJSONString" ] .map (method => attempt (() => value [method] ()));

      return {
         sfnode: forms (M),
         transform: forms (T),
         children: forms (T .children),
         first: forms (T .children [0]),
         roots: forms (scene .rootNodes),
         empty: forms (scene .getNamedNode ("G") .children),
         nul: forms (new X3D .SFNode ()),
         field: forms (T .getField ("children")),
         sfield: forms (T .children [0] .getField ("appearance")),
      };
   });
}

// X3DField.fromString, fromVRMLString and fromXMLString of every kind of field, with and without a scene.
async function fromString (page)
{
   await load (page);

   return page .evaluate (() =>
   {
      const X3D   = window .X3D_NS;
      const scene = document .querySelector ("x3d-canvas") .browser .currentScene;
      const read  = (type, string, method = "fromString", ... rest) => attempt (() => { const field = new X3D [type] (); field [method] (string, ... rest); return String (field); });
      const out   = { };

      out .scalars = [
         read ("SFBool", "TRUE"), read ("SFBool", "false"), read ("SFBool", "x"),
         read ("SFFloat", "1.5"), read ("SFDouble", "-2e3"), read ("SFInt32", "0x10"), read ("SFInt32", "abc"),
         read ("SFString", "\"a \\\"b\\\"\""), read ("SFString", "plain"), read ("SFTime", "3.25"),
      ];

      out .vectors = [
         read ("SFVec2f", "1 2"), read ("SFVec3f", "1 2 3"), read ("SFVec3d", "1.5 2.5 3.5"), read ("SFVec4f", "1 2 3 4"),
         read ("SFVec3f", "1 2"), read ("SFColor", "0.1 0.2 0.3"), read ("SFColorRGBA", "0.1 0.2 0.3 0.4"),
         read ("SFRotation", "0 1 0 1.5"), read ("SFRotation", "0 0 0 1"),
         read ("SFMatrix3f", "1 0 0 0 1 0 0 0 1"), read ("SFMatrix4f", "1 0 0 0 0 1 0 0 0 0 1 0 1 2 3 1"),
         read ("SFQuaternion", "0 0 0 1"),
      ];

      out .arrays = [
         read ("MFFloat", "[ 1 2 3 ]"), read ("MFFloat", "1, 2, 3"), read ("MFInt32", "1 2 3"), read ("MFInt32", "[ ]"),
         read ("MFString", "\"a\" \"b\""), read ("MFString", "[ \"a\", \"b\" ]"), read ("MFString", "a"),
         read ("MFVec3f", "[ 1 2 3, 4 5 6 ]"), read ("MFVec3f", "1 2 3 4 5 6"), read ("MFVec3f", "1 2 3 4"),
         read ("MFRotation", "[ 0 1 0 1, 1 0 0 2 ]"), read ("MFColor", "[ 1 0 0, 0 1 0 ]"), read ("MFBool", "[ TRUE FALSE ]"),
         read ("MFDouble", "[ 1.5 2.5 ]"), read ("MFTime", "[ 1 2 ]"), read ("MFMatrix4f", "[ 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 ]"),
      ];

      out .images = [
         read ("SFImage", "2 1 3 0xff0000 0x00ff00"), read ("SFImage", "1 1 1 0x80"), read ("SFImage", "0 0 0"),
         read ("MFImage", "1 1 1 0x80 2 1 1 0x01 0x02"), read ("SFImage", "2 2 3 0xff"),
      ];

      out .methods = [
         read ("SFVec3f", "1 2 3", "fromVRMLString"), read ("SFVec3f", "1 2 3", "fromXMLString"), read ("SFVec3f", "1, 2, 3", "fromXMLString"),
         read ("MFString", "\"a\" \"b\"", "fromXMLString"), read ("MFString", "a b", "fromXMLString"),
         read ("SFBool", "true", "fromXMLString"), read ("SFBool", "TRUE", "fromXMLString"),
      ];

      // The nodes of a value: with a scene they are made in it; without one the parser has no execution context.
      const nodes = (type, string, method = "fromString") => attempt (() =>
      {
         const field = new X3D [type] ();

         field [method] (string, scene);

         return [ String (field), type === "SFNode" ? field .valueOf () ?.getNodeTypeName () ?? null : Array .from (field, n => n ?.getNodeTypeName ()) ];
      });

      out .nodes = [
         nodes ("SFNode", "Transform { }"), nodes ("SFNode", "NULL"), nodes ("SFNode", "DEF X Material { }"), nodes ("SFNode", "USE M"),
         nodes ("MFNode", "[ Group { } Transform { } ]"), nodes ("MFNode", "Group { }"), nodes ("MFNode", "[ ]"), nodes ("SFNode", "Bogus { }"),
         nodes ("SFNode", "<Transform/>", "fromXMLString"), nodes ("MFNode", "<Group/><Transform/>", "fromXMLString"),
         attempt (() => { const f = new X3D .SFNode (); f .fromString ("Transform { }"); return String (f); }),
         attempt (() => { const f = new X3D .SFNode (); f .fromString ("NULL"); return String (f); }),
         attempt (() => { const f = new X3D .MFNode (); f .fromString ("[ ]"); return String (f); }),
         attempt (() => { const f = new X3D .MFNode (); f .fromString ("Group { }"); return String (f); }),
      ];

      // A field of a node: the value goes through setValue and reaches the node.
      const T = scene .getNamedNode ("T");

      T .getField ("translation") .fromString ("4 5 6");
      T .getField ("rotation") .fromString ("0 0 1 3");
      out .node = [ String (T .translation), String (T .rotation), String (T .getField ("translation") .getName ()) ];

      return out;
   });
}

const cases = { text, fromString };

for (const [ name, run ] of Object .entries (cases))
   cases [name] = async (page, env) => { await page .evaluate (pageAttempt); return run (page, env); };

export const CASE_GROUPS = {
   fieldText: { context: DESKTOP, cases },
};
