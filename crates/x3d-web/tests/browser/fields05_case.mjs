// The field API 05 cases (issue #137): PROTO and EXTERNPROTO declarations of an X3DScene. The declaration objects
// (X3DProtoDeclaration, X3DExternProtoDeclaration: name, interface, body, isExternProto, loadState, newInstance, loadNow,
// the text forms), the live ProtoDeclarationArray and ExternProtoDeclarationArray (scene.protos, scene.externprotos), and
// getProtoDeclaration, addProtoDeclaration, updateProtoDeclaration, removeProtoDeclaration, their EXTERNPROTO forms and
// getUniqueProtoName / getUniqueExternProtoName. oracle/x_ite-web-runtime-02.mjs (--suite fields05) runs them against pinned
// X_ITE 16.5.0 and records the values; fields05.test.mjs runs them against X_ITE_R and compares (runtime02_harness.mjs).
// The worlds are synthetic and inline (`data:` URLs).

import { DESKTOP, settle } from "./runtime02_case.mjs";
import { pageLoadURL } from "./loadurl01_case.mjs";

const EXTERN_FILE = `data:model/x3d-vrml,${encodeURIComponent ("#X3D V4.0 utf8\nPROFILE Full\nPROTO Far [ inputOutput SFFloat size 3 inputOnly SFBool go ] { Group { } }\nPROTO Other [ ] { Group { } }\nGroup { }\n")}`;

const WORLD = `data:model/x3d-vrml,${encodeURIComponent (`#X3D V4.0 utf8
PROFILE Full
META "title" "declarations"
PROTO Pair [
   inputOutput SFFloat amount 1.5
   inputOutput SFString label "x"
   inputOnly SFBool go
   outputOnly SFInt32 count
   initializeOnly MFVec3f points [ 0 0 0, 1 1 1 ]
]
{
   DEF Inner Transform { translation 1 2 3 children Shape { appearance Appearance { material Material { transparency IS amount } } geometry Sphere { } } }
}
PROTO Plain [ ] { Group { } }
EXTERNPROTO Far [ inputOutput SFFloat size, inputOnly SFBool go ] [ "${EXTERN_FILE}#Far", "${EXTERN_FILE}" ]
EXTERNPROTO Gone [ inputOutput SFInt32 n ] "${EXTERN_FILE}#Missing"
DEF G Group { children Pair { amount 0.5 } }
`)}`;

async function load (page, url = WORLD)
{
   await page .evaluate (() => { window .attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : v; } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } }; });
   await page .evaluate (pageLoadURL, [ url ]);

   const result = await page .evaluate (() => window .pending);

   if (result !== "resolved")
      throw new Error (`the world did not load: ${result}\n${page .log .join ("\n")}`);

   await settle (page, 100);
}

// The page's `shape (value)`: primitives as they are, objects as their class name (and the text of a field or an array's names).
function pageShape ()
{
   window .shape = value =>
   {
      if (value === null || value === undefined || typeof value !== "object")
         return typeof value === "function" ? "function" : value === undefined ? "undefined" : value;

      return value .constructor ?.name ?? Object .prototype .toString .call (value);
   };
}

const logLines = page => page .log .filter (line => !/^(log|info|debug):/ .test (line));

// scene .protos, scene .externprotos and the getters.
async function lists (page)
{
   await page .evaluate (pageShape);
   await load (page);

   const out = await page .evaluate (() =>
   {
      const scene   = document .querySelector ("x3d-canvas") .browser .currentScene;
      const attempt = window .attempt;
      const shape   = window .shape;
      const out     = { };

      out .protos = [ shape (scene .protos), scene .protos .length, scene .protos === scene .protos, scene .getProtoDeclarations () === scene .protos,
         attempt (() => Array .from (scene .protos, p => p .name)), attempt (() => Array .from (scene .protos, p => shape (p))),
         attempt (() => scene .protos .has ("Pair")), attempt (() => scene .protos .has ("Far")), attempt (() => shape (scene .protos .get ("Plain"))),
         attempt (() => scene .protos .get ("Nope")), attempt (() => shape (scene .protos [0])), attempt (() => shape (scene .protos [9])) ];
      out .externprotos = [ shape (scene .externprotos), scene .externprotos .length, scene .externprotos === scene .externprotos,
         scene .getExternProtoDeclarations () === scene .externprotos, attempt (() => Array .from (scene .externprotos, p => p .name)),
         attempt (() => scene .externprotos .has ("Far")), attempt (() => scene .externprotos .has ("Pair")) ];
      out .sameObject = [ scene .protos [0] === scene .getProtoDeclaration ("Pair"), scene .externprotos [0] === scene .getExternProtoDeclaration ("Far") ];
      out .getters = {
         proto: attempt (() => shape (scene .getProtoDeclaration ("Pair"))),
         unknown: attempt (() => scene .getProtoDeclaration ("Nope")),
         unknownExtern: attempt (() => scene .getExternProtoDeclaration ("Nope")),
         externAsProto: attempt (() => scene .getProtoDeclaration ("Far")),
         protoAsExtern: attempt (() => scene .getExternProtoDeclaration ("Pair")),
         empty: attempt (() => scene .getProtoDeclaration ("")),
         none: attempt (() => scene .getProtoDeclaration ()),
         number: attempt (() => scene .getProtoDeclaration (5)),
      };
      out .unique = [ scene .getUniqueProtoName ("Pair"), scene .getUniqueProtoName ("Free"), scene .getUniqueProtoName ("Far"), scene .getUniqueProtoName (""),
         scene .getUniqueExternProtoName ("Far"), scene .getUniqueExternProtoName ("Pair"), scene .getUniqueExternProtoName ("Free"),
         attempt (() => scene .getUniqueProtoName ()), attempt (() => scene .getUniqueProtoName ("Pair_1")), attempt (() => scene .getUniqueProtoName ("Pair2")) ];
      out .array = [ attempt (() => scene .protos .length = 0), attempt (() => scene .protos .push), attempt (() => scene .protos .add), attempt (() => Object .getOwnPropertyNames (Object .getPrototypeOf (scene .protos)) .sort ()) ];
      out .ownKeys = attempt (() => Object .keys (scene .protos));
      out .text = attempt (() => scene .toVRMLString () .split ("\n") .filter (l => /^(PROTO|EXTERNPROTO)/ .test (l)));

      return out;
   });

   out .log = logLines (page);

   return out;
}

// The members of X3DProtoDeclaration and X3DExternProtoDeclaration objects.
async function declaration (page)
{
   await page .evaluate (pageShape);
   await load (page);

   const out = await page .evaluate (async () =>
   {
      const scene   = document .querySelector ("x3d-canvas") .browser .currentScene;
      const attempt = window .attempt;
      const shape   = window .shape;
      const X3D     = window .X3D_NS;
      const pair    = scene .getProtoDeclaration ("Pair");
      const plain   = scene .getProtoDeclaration ("Plain");
      const out     = { };

      const fields = decl => attempt (() => Array .from (decl .getFieldDefinitions (), d => [ d .name, d .accessType, d .dataType, shape (d .value), String (d .value) ]));

      out .pair = {
         type: [ shape (pair), pair .isExternProto, pair .name, attempt (() => pair .getTypeName ()), attempt (() => pair .getType ()), attempt (() => pair .getNodeTypeName ?.()) ],
         context: [ pair .getExecutionContext () === scene, attempt (() => pair .getBrowser () === scene .getBrowser ()) ],
         fields: fields (pair), fieldsLength: attempt (() => pair .fields .length), fieldsSame: attempt (() => pair .fields === pair .getFieldDefinitions ()),
         userDefined: attempt (() => Array .from (pair .getUserDefinedFields (), f => f .getName ())),
         getField: [ attempt (() => String (pair .getField ("amount") ?.getValue ())), attempt (() => pair .getField ("nope")) ],
         protoDeclaration: attempt (() => pair .getProtoDeclaration () === pair),
         appInfo: [ attempt (() => pair .getAppInfo ()), attempt (() => pair .appInfo), attempt (() => { pair .setAppInfo ("info"); return pair .getAppInfo (); }), attempt (() => { pair .documentation = "doc"; return pair .getDocumentation (); }) ],
         canUserDefinedFields: attempt (() => pair .canUserDefinedFields ()),
         cloneCount: attempt (() => pair .getCloneCount ()),
      };

      const body = attempt (() => pair .getBody ());

      out .body = {
         same: attempt (() => pair .body === pair .getBody ()),
         type: shape (body),
         rootNodes: attempt (() => body .rootNodes .length),
         first: attempt (() => body .rootNodes [0] .getNodeTypeName ()),
         named: attempt (() => body .getNamedNode ("Inner") .getNodeTypeName ()),
         namedList: attempt (() => Array .from (body .namedNodes, n => n .getNodeName ?.() ?? n .name)),
         outer: attempt (() => shape (body .getOuterNode ())),
         scene: attempt (() => shape (body .getLocalScene ())),
         isScene: attempt (() => body .isScene),
         text: attempt (() => body .toVRMLString ()),
         plainBody: attempt (() => plain .body .rootNodes .length),
      };

      out .text = {
         vrml: attempt (() => pair .toVRMLString ()),
         xml: attempt (() => pair .toXMLString ()),
         json: attempt (() => pair .toJSONString ()),
         plain: attempt (() => plain .toVRMLString ()),
         extern: attempt (() => scene .getExternProtoDeclaration ("Gone") .toVRMLString ()),
         externXml: attempt (() => scene .getExternProtoDeclaration ("Gone") .toXMLString ()),
         externJson: attempt (() => scene .getExternProtoDeclaration ("Gone") .toJSONString ()),
      };

      const instance = attempt (() => pair .newInstance ());

      out .instance = {
         shape: shape (instance),
         typeName: attempt (() => instance .getNodeTypeName ()),
         amount: attempt (() => instance .amount),
         inRoot: attempt (() => scene .rootNodes .length),
         create: attempt (() => pair .createInstance (scene) .getNodeTypeName ()),
         createNoContext: attempt (() => pair .createInstance ()),
         plain: attempt (() => plain .newInstance () .getNodeTypeName ()),
         isInstance: attempt (() => instance .getType () .includes (X3D .X3DConstants .X3DPrototypeInstance)),
         protoOfInstance: attempt (() => instance .getProtoNode ?.() === pair),
      };

      out .constants = [ attempt (() => pair .getType () .includes (X3D .X3DConstants .X3DProtoDeclaration)), attempt (() => pair .getType () .includes (X3D .X3DConstants .X3DProtoDeclarationNode)) ];
      out .classes = [ typeof X3D .X3DProtoDeclaration, typeof X3D .X3DExternProtoDeclaration, typeof X3D .ProtoDeclarationArray, typeof X3D .ExternProtoDeclarationArray ];

      return out;
   });

   out .log = logLines (page);

   return out;
}

// add, update and remove (Extern)ProtoDeclaration, their errors and the text form after each.
async function operations (page)
{
   await page .evaluate (pageShape);
   await load (page);

   const out = await page .evaluate (async () =>
   {
      const scene   = document .querySelector ("x3d-canvas") .browser .currentScene;
      const attempt = window .attempt;
      const shape   = window .shape;
      const out     = { };
      const names   = () => [ Array .from (scene .protos, p => p .name), Array .from (scene .externprotos, p => p .name) ];
      const pair    = scene .getProtoDeclaration ("Pair");
      const plain   = scene .getProtoDeclaration ("Plain");
      const far     = scene .getExternProtoDeclaration ("Far");
      const gone    = scene .getExternProtoDeclaration ("Gone");

      out .addErrors = [
         attempt (() => scene .addProtoDeclaration ("Pair", pair)), attempt (() => scene .addProtoDeclaration ("Fresh", pair)),
         attempt (() => scene .addProtoDeclaration ("Fresh", far)), attempt (() => scene .addProtoDeclaration ("Fresh", null)),
         attempt (() => scene .addProtoDeclaration ("Fresh")), attempt (() => scene .addProtoDeclaration ("", plain)),
         attempt (() => scene .addExternProtoDeclaration ("Far", far)), attempt (() => scene .addExternProtoDeclaration ("Fresh", far)),
         attempt (() => scene .addExternProtoDeclaration ("Fresh", pair)), names (),
      ];
      out .updateErrors = [
         attempt (() => scene .updateProtoDeclaration ("Fresh", null)), attempt (() => scene .updateProtoDeclaration ("", plain)),
         attempt (() => scene .updateProtoDeclaration ("Fresh", far)), attempt (() => scene .updateExternProtoDeclaration ("Fresh", pair)), names (),
      ];

      // Rename by update; the declaration object stays the same and the name moves with it.
      out .update = [ attempt (() => scene .updateProtoDeclaration ("Renamed", plain)), names (), plain .name, scene .getProtoDeclaration ("Renamed") === plain,
         attempt (() => scene .getProtoDeclaration ("Plain")), attempt (() => Array .from (scene .protos, p => p .name)) ];
      out .updateSame = [ attempt (() => scene .updateProtoDeclaration ("Renamed", plain)), names () ];
      out .updateOver = [ attempt (() => scene .updateProtoDeclaration ("Pair", plain)), names () ];
      out .updateExtern = [ attempt (() => scene .updateExternProtoDeclaration ("FarRenamed", far)), names (), far .name ];
      out .afterText = attempt (() => scene .toVRMLString () .split ("\n") .filter (l => /^(PROTO|EXTERNPROTO)/ .test (l)));

      // Remove, and add the removed declaration again.
      out .remove = [ attempt (() => scene .removeProtoDeclaration ("Pair")), names (), attempt (() => scene .removeProtoDeclaration ("Pair")), attempt (() => scene .removeProtoDeclaration ()),
         attempt (() => scene .removeExternProtoDeclaration ("Gone")), names (), attempt (() => scene .removeExternProtoDeclaration ("Nope")) ];
      out .stillUsable = [ attempt (() => pair .name), attempt (() => pair .newInstance () .getNodeTypeName ()), attempt (() => pair .toVRMLString () .split ("\n") [0]),
         attempt (() => gone .name), attempt (() => scene .getProtoDeclaration ("Pair")) ];
      out .readd = [ attempt (() => scene .addProtoDeclaration ("Pair", pair)), attempt (() => scene .addProtoDeclaration ("Again", pair)), names (), pair .name ];
      out .readdExtern = [ attempt (() => scene .addExternProtoDeclaration ("Gone2", gone)), names (), gone .name ];

      // A name that a node type of the profile has, and the name of the other kind.
      out .warnings = [ attempt (() => scene .updateProtoDeclaration ("Group", plain)), attempt (() => scene .updateProtoDeclaration ("Renamed", plain)),
         attempt (() => scene .updateExternProtoDeclaration ("Pair", gone)), attempt (() => scene .updateExternProtoDeclaration ("Gone2", gone)),
         attempt (() => scene .updateProtoDeclaration ("FarRenamed", plain)), names () ];
      out .instances = [ attempt (() => scene .createProto ("Pair") ?.getNodeTypeName ()), attempt (() => scene .createProto ("FarRenamed") ?.getNodeTypeName ()), attempt (() => scene .createProto ("Renamed")) ];
      out .text = attempt (() => scene .toVRMLString () .split ("\n") .filter (l => /^(PROTO|EXTERNPROTO)/ .test (l)));

      // A declaration of another scene.
      const other = await window .X3D_NS .getBrowser (document .querySelector ("x3d-canvas")) .createX3DFromString ("#X3D V4.0 utf8\nPROFILE Full\nPROTO Foreign [ ] { Group { } }\nEXTERNPROTO FarOther [ ] \"x.wrl\"\nGroup { }\n");
      const foreign = other .getProtoDeclaration ("Foreign");

      out .foreign = [ attempt (() => scene .addProtoDeclaration ("ForeignAdd", foreign)), attempt (() => scene .updateProtoDeclaration ("Renamed", foreign)),
         attempt (() => scene .addExternProtoDeclaration ("FarX", other .getExternProtoDeclaration ("FarOther"))), attempt (() => foreign .getExecutionContext () === other), names () ];

      return out;
   });

   out .log = logLines (page);

   return out;
}

// EXTERNPROTO declarations: the load state, the loaded PROTO, loadNow and the interface.
async function externProtos (page)
{
   await page .evaluate (pageShape);
   await load (page);

   const out = await page .evaluate (async () =>
   {
      const X3D     = window .X3D_NS;
      const scene   = document .querySelector ("x3d-canvas") .browser .currentScene;
      const attempt = window .attempt;
      const shape   = window .shape;
      const far     = scene .getExternProtoDeclaration ("Far");
      const gone    = scene .getExternProtoDeclaration ("Gone");
      const state   = decl => Object .entries (X3D .X3DConstants) .find (([ , v ]) => v === decl .loadState) ?.[0];
      const out     = { };

      out .far = {
         type: [ shape (far), far .isExternProto, far .name, attempt (() => far .getTypeName ()), state (far) ],
         urls: attempt (() => Array .from (far .urls)), urlsShape: attempt (() => shape (far .urls)),
         fields: attempt (() => Array .from (far .getFieldDefinitions (), d => [ d .name, d .accessType, d .dataType ])),
         proto: attempt (() => shape (far .getProtoDeclaration ())), protoName: attempt (() => far .getProtoDeclaration () ?.name),
         protoSame: attempt (() => far .getProtoDeclaration () === scene .getProtoDeclaration ("Far")),
         instance: attempt (() => far .newInstance () .getNodeTypeName ()),
         text: attempt (() => far .toVRMLString ()), xml: attempt (() => far .toXMLString ()), json: attempt (() => far .toJSONString ()),
         appInfo: attempt (() => far .getAppInfo ()),
      };
      out .gone = {
         state: state (gone), urls: attempt (() => Array .from (gone .urls)), proto: attempt (() => gone .getProtoDeclaration ()),
         instance: attempt (() => gone .newInstance () .getNodeTypeName ()),
         text: attempt (() => gone .toVRMLString ()),
      };
      out .members = [ typeof far .loadNow, typeof far .unloadNow, attempt (() => far .load), attempt (() => far .autoRefresh), attempt (() => far .autoRefreshTimeLimit), attempt (() => String (far .getField ("url") .getValue ())) ];

      // loadNow loads again, unloadNow takes the loaded PROTO away.
      const loaded = new Promise (resolve => { const check = () => state (far) === "COMPLETE_STATE" || state (far) === "FAILED_STATE" ? resolve (state (far)) : setTimeout (check, 20); check (); });

      out .loadNow = [ attempt (() => far .loadNow ()), state (far), await loaded, shape (far .getProtoDeclaration ()) ];
      out .unloadNow = [ attempt (() => far .unloadNow ()), state (far), shape (far .getProtoDeclaration ()) ];
      out .goneLoadNow = [ attempt (() => gone .loadNow ()), state (gone) ];

      return out;
   });

   await settle (page, 100);
   out .log = logLines (page);

   return out;
}

// addUserDefinedField and removeUserDefinedField of a PROTO declaration, updateInstances and requestUpdateInstances.
async function interfaceChanges (page)
{
   await page .evaluate (pageShape);
   await load (page);

   const out = await page .evaluate (async () =>
   {
      const X3D     = window .X3D_NS;
      const scene   = document .querySelector ("x3d-canvas") .browser .currentScene;
      const attempt = window .attempt;
      const out     = { };
      const pair    = scene .getProtoDeclaration ("Pair");
      const inst    = scene .getNamedNode ("G") .children [0];
      const names   = decl => Array .from (decl .getFieldDefinitions (), d => d .name);

      out .before = [ names (pair), attempt (() => Array .from (inst .getFieldDefinitions (), d => d .name)) ];
      out .add = [ attempt (() => pair .addUserDefinedField (X3D .X3DConstants .inputOutput, "extra", new X3D .SFVec3f (1, 2, 3))), names (pair),
         attempt (() => pair .addUserDefinedField (X3D .X3DConstants .inputOutput, "amount", new X3D .SFFloat (9))), names (pair),
         attempt (() => pair .addUserDefinedField (X3D .X3DConstants .inputOutput, "bad")), attempt (() => scene .getExternProtoDeclaration ("Far") .addUserDefinedField (X3D .X3DConstants .inputOutput, "e", new X3D .SFBool ())) ];
      out .instanceBefore = attempt (() => Array .from (inst .getFieldDefinitions (), d => d .name));
      out .update = [ attempt (() => pair .updateInstances ()), attempt (() => Array .from (inst .getFieldDefinitions (), d => d .name)), attempt (() => String (inst .extra)) ];
      out .request = [ attempt (() => pair .addUserDefinedField (X3D .X3DConstants .initializeOnly, "later", new X3D .SFString ("v"))), attempt (() => pair .requestUpdateInstances ()) ];
      await new Promise (resolve => setTimeout (resolve, 300));
      out .requested = [ attempt (() => Array .from (inst .getFieldDefinitions (), d => d .name)), attempt (() => String (inst .later)) ];
      out .remove = [ attempt (() => pair .removeUserDefinedField ("extra")), attempt (() => pair .removeUserDefinedField ("nope")), names (pair), attempt (() => pair .updateInstances ()), attempt (() => Array .from (inst .getFieldDefinitions (), d => d .name)) ];
      out .text = attempt (() => pair .toVRMLString ());
      out .newInstance = attempt (() => Array .from (pair .newInstance () .getFieldDefinitions (), d => d .name));

      return out;
   });

   out .log = logLines (page);

   return out;
}

export const CASE_GROUPS = {
   protoDeclarations: { context: DESKTOP, cases: { lists, declaration, operations, externProtos, interfaceChanges } },
};
