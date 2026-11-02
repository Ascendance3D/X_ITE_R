// The DOM integration 02 cases (issue #146, part 2): the world follows the <X3D> element after it was imported. X_ITE's
// DOMIntegration observes the element (attributes, childList, subtree): an attribute is read into the field of its node, an added
// element is parsed into the context of its parent, a removed element's node is disposed. The cases change real elements of a
// real page and read the scene after each change (oracle/x_ite-web-runtime-02.mjs --suite dom02 records pinned X_ITE 16.5.0;
// dom02.test.mjs compares X_ITE_R, runtime02_harness.mjs). The worlds are synthetic.

import { DESKTOP, settle } from "./runtime02_case.mjs";

const logLines = page => page .log .filter (line => !/^(log|info|debug):/ .test (line)) .map (line => line .replace (/127\.0\.0\.1:\d+/g, "HOST") .replace (/HOST\/(?:x_ite\/src|tests\/browser)\//g, "HOST/PAGE/") .replace (/(?:__x_ite_r_web_runtime_02__|runtime02)\.html/g, "page.html"));

async function mount (page, html, ms = 600)
{
   await page .evaluate (html => { document .querySelector ("x3d-canvas") .innerHTML = html; }, html);
   await settle (page, ms);
}

// The scene after a change: its text, the roots, the names, the routes and the sampled fields.
const READ = names =>
{
   const browser = document .querySelector ("x3d-canvas") .browser;
   const scene   = browser .currentScene;
   const attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : v; } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };

   return {
      vrml: attempt (() => scene .toVRMLString ()),
      roots: attempt (() => Array .from (scene .rootNodes, n => n ?.getNodeTypeName () ?? null)),
      named: attempt (() => Array .from (scene .namedNodes, n => n .getNodeName ())),
      routes: attempt (() => Array .from (scene .routes, r => `${r .sourceNode .getNodeName ()}.${r .sourceField} -> ${r .destinationNode .getNodeName ()}.${r .destinationField}`)),
      fields: Object .fromEntries (Object .entries (names) .map (([ key, [ node, field ] ]) => [ key, attempt (() => String (scene .getNamedNode (node) [field])) ])),
   };
};

// Run the changes one by one; each is a function of the page, given the <X3D> element.
async function run (page, world, steps, names = { })
{
   await mount (page, world);

   const result = [ [ "start", await page .evaluate (READ, names) ] ];

   for (const [ label, change ] of steps)
   {
      const error = await page .evaluate (async change =>
      {
         try
         {
            new Function ("x3d", "document", change) (document .querySelector ("x3d-canvas > x3d"), document);
         }
         catch (error)
         {
            return `${error ?.constructor ?.name}: ${error ?.message}`;
         }

         return null;
      }, change);

      await settle (page, 400);
      result .push ([ label, error, await page .evaluate (READ, names) ]);
   }

   return { steps: result, log: logLines (page) };
}

const q = (selector, body) => `const e = x3d .querySelector (${JSON .stringify (selector)}); ${body}`;

const BASE = `<X3D profile='Full'><Scene>
<Transform DEF='T' translation='1 2 3'><Shape DEF='S'><Appearance><Material DEF='M' diffuseColor='1 0 0'></Material></Appearance><Box DEF='B' size='2 2 2'></Box></Shape></Transform>
<Group DEF='G'><Sphere radius='0.5'></Sphere></Group>
</Scene></X3D>`;

const NAMES = { translation: [ "T", "translation" ], diffuse: [ "M", "diffuseColor" ], size: [ "B", "size" ] };

// A changed attribute is read into the field of the node of the element.
function attributes (page)
{
   return run (page, BASE, [
      [ "translation", q ("[def=T]", "e .setAttribute ('translation', '4 5 6')") ],
      [ "upperName", q ("[def=T]", "e .setAttribute ('TRANSLATION', '7 8 9')") ],
      [ "color", q ("[def=M]", "e .setAttribute ('diffuseColor', '0 1 0')") ],
      [ "colorLower", q ("[def=M]", "e .setAttribute ('diffusecolor', '0 0 1')") ],
      [ "bad", q ("[def=B]", "e .setAttribute ('size', 'a b c')") ],
      [ "short", q ("[def=B]", "e .setAttribute ('size', '1 2')") ],
      [ "unknown", q ("[def=B]", "e .setAttribute ('nothing', '1')") ],
      [ "outputOnly", q ("[def=T]", "e .setAttribute ('bboxCenter', '1 1 1'); e .setAttribute ('children_changed', '1')") ],
      [ "remove", q ("[def=T]", "e .removeAttribute ('translation')") ],
      [ "same", q ("[def=T]", "e .setAttribute ('translation', '4 5 6'); e .setAttribute ('translation', '4 5 6')") ],
      [ "many", q ("[def=B]", "e .setAttribute ('size', '1 1 1'); e .setAttribute ('size', '3 3 3')") ],
      [ "defChange", q ("[def=G]", "e .setAttribute ('DEF', 'Renamed')") ],
      [ "useChange", q ("[def=S]", "e .setAttribute ('USE', 'S')") ],
      [ "container", q ("[def=B]", "e .setAttribute ('containerField', 'appearance')") ],
      [ "outside", "x3d .setAttribute ('profile', 'Core')" ],
   ], NAMES);
}

// Added elements are parsed into the context of their parent.
function adding (page)
{
   return run (page, BASE, [
      [ "intoGroup", q ("[def=G]", "e .insertAdjacentHTML ('beforeend', '<Box DEF=\"B2\" size=\"4 4 4\"></Box>')") ],
      [ "intoScene", q ("scene", "e .insertAdjacentHTML ('beforeend', '<Group DEF=\"Late\"><Cone></Cone></Group>')") ],
      [ "tree", q ("[def=T]", "e .insertAdjacentHTML ('beforeend', '<Group DEF=\"Deep\"><Transform DEF=\"Deeper\"><Shape><Cylinder></Cylinder></Shape></Transform></Group>')") ],
      [ "upper", q ("[def=G]", "e .insertAdjacentHTML ('beforeend', '<TRANSFORM DEF=\"Up\" TRANSLATION=\"1 1 1\"><SHAPE><BOX></BOX></SHAPE></TRANSFORM>')") ],
      [ "useAdded", q ("[def=G]", "e .insertAdjacentHTML ('beforeend', '<Shape USE=\"S\"></Shape>')") ],
      [ "unknownNode", q ("[def=G]", "e .insertAdjacentHTML ('beforeend', '<Nothing></Nothing>')") ],
      [ "text", q ("[def=G]", "e .append ('text', document .createComment ('c'))") ],
      [ "toSceneNotShape", q ("[def=G]", "e .insertAdjacentHTML ('beforeend', '<Material></Material>')") ],
      [ "outsideScene", "x3d .insertAdjacentHTML ('beforeend', '<Group DEF=\"Out\"></Group>')" ],
      [ "newScene", "x3d .insertAdjacentHTML ('beforeend', '<Scene><Group DEF=\"InNew\"></Group></Scene>')" ],
      [ "inDiv", "x3d .insertAdjacentHTML ('beforeend', '<div><Group DEF=\"InDiv\"></Group></div>')" ],
      [ "head", "x3d .insertAdjacentHTML ('afterbegin', '<head><meta name=\"a\" content=\"b\"></head>')" ],
      [ "moved", q ("[def=B2]", "x3d .querySelector ('scene') .append (e)") ],
   ]);
}

// A removed element's node is disposed.
function removing (page)
{
   return run (page, BASE + "", [
      [ "shape", q ("[def=S]", "e .remove ()") ],
      [ "group", q ("[def=G]", "e .remove ()") ],
      [ "transform", q ("[def=T]", "e .remove ()") ],
      [ "all", q ("scene", "e .replaceChildren ()") ],
   ], { translation: [ "T", "translation" ] });
}

// ROUTE elements and nodes with routes, fieldValue and ProtoInstance elements.
function routes (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<TimeSensor DEF='Clock' cycleInterval='4' enabled='false'></TimeSensor>
<PositionInterpolator DEF='Move' key='0 1' keyValue='0 0 0, 1 1 1'></PositionInterpolator>
<Transform DEF='Box'></Transform>
<ProtoDeclare name='P'><ProtoInterface><field accessType='inputOutput' type='SFFloat' name='amount' value='1'></field></ProtoInterface><ProtoBody><Group DEF='Inner'></Group></ProtoBody></ProtoDeclare>
<ProtoInstance name='P' DEF='I'><fieldValue name='amount' value='2'></fieldValue></ProtoInstance>
<ROUTE fromNode='Clock' fromField='fraction_changed' toNode='Move' toField='set_fraction'></ROUTE>
</Scene></X3D>`, [
      [ "addRoute", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<ROUTE fromNode=\"Move\" fromField=\"value_changed\" toNode=\"Box\" toField=\"set_translation\"></ROUTE>')" ],
      [ "badRoute", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<ROUTE fromNode=\"Nope\" fromField=\"a\" toNode=\"Box\" toField=\"set_translation\"></ROUTE>')" ],
      [ "removeRoute", q ("route", "e .remove ()") ],
      [ "fieldValue", q ("fieldvalue", "e .setAttribute ('value', '5')") ],
      [ "addFieldValue", q ("[def=I]", "e .insertAdjacentHTML ('beforeend', '<fieldValue name=\"amount\" value=\"7\"></fieldValue>')") ],
      [ "addInstance", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<ProtoInstance name=\"P\" DEF=\"I2\"></ProtoInstance>')" ],
      [ "removeNodeWithRoute", q ("[def=Move]", "e .remove ()") ],
      [ "removeInstance", q ("[def=I]", "e .remove ()") ],
   ], { amount: [ "I", "amount" ] });
}

// Script, ProtoDeclare and ExternProtoDeclare elements added after the import.
function declarations (page)
{
   return run (page, `<X3D profile='Full'><Scene><Group DEF='G'></Group></Scene></X3D>`, [
      [ "proto", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<ProtoDeclare name=\"Q\"><ProtoInterface><field accessType=\"initializeOnly\" type=\"SFBool\" name=\"on\" value=\"true\"></field></ProtoInterface><ProtoBody><Group DEF=\"QI\"></Group></ProtoBody></ProtoDeclare>')" ],
      [ "instance", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<ProtoInstance name=\"Q\" DEF=\"Qi\"></ProtoInstance>')" ],
      [ "script", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<Script DEF=\"Sc\"><field name=\"a\" type=\"SFInt32\" accessType=\"inputOutput\" value=\"3\"></field><![CDATA[ecmascript:\\nfunction initialize () { a = 4; }\\n]]></Script>')" ],
      [ "scriptField", q ("[def=Sc] field", "e .setAttribute ('value', '9')") ],
      [ "extern", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<ExternProtoDeclare name=\"Ex\" url=\"\"></ExternProtoDeclare>')" ],
      [ "removeProto", q ("protodeclare", "e .remove ()") ],
   ]);
}

// The changes are live: an attribute sends its event through a ROUTE, an added node can be routed to, a removed node's routes
// are gone and a removed node no longer receives events.
function live (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<Transform DEF='A'></Transform>
<Transform DEF='B'></Transform>
<ROUTE fromNode='A' fromField='translation_changed' toNode='B' toField='set_translation'></ROUTE>
</Scene></X3D>`, [
      [ "attribute", q ("[def=A]", "e .setAttribute ('translation', '1 2 3')") ],
      [ "add", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<Transform DEF=\"C\"></Transform><ROUTE fromNode=\"A\" fromField=\"translation_changed\" toNode=\"C\" toField=\"set_translation\"></ROUTE>')" ],
      [ "again", q ("[def=A]", "e .setAttribute ('translation', '4 5 6')") ],
      [ "removeB", q ("[def=B]", "e .remove ()") ],
      [ "afterRemove", q ("[def=A]", "e .setAttribute ('translation', '7 8 9')") ],
      [ "scale", q ("[def=C]", "e .setAttribute ('scale', '2 2 2'); e .setAttribute ('rotation', '0 0 1 1')") ],
   ], { a: [ "A", "translation" ], b: [ "B", "translation" ], c: [ "C", "translation" ], cscale: [ "C", "scale" ] });
}

export const CASE_GROUPS = {
   domMutation: { context: DESKTOP, cases: { attributes, adding, removing, routes, declarations, live } },
};
