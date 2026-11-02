// The DOM integration 03 cases (issue #146, part 3): the CustomEvents of the output fields of the nodes of an <X3D> element, and
// its Inline elements (the loaded scene becomes the child of the element; `load` and `error` events). oracle/x_ite-web-runtime-02.mjs
// (--suite dom03) records pinned X_ITE 16.5.0; dom03.test.mjs compares X_ITE_R (runtime02_harness.mjs). The worlds are synthetic.

import { DESKTOP, settle } from "./runtime02_case.mjs";

const logLines = page => page .log .filter (line => !/^(log|info|debug):/ .test (line)) .map (line => line .replace (/127\.0\.0\.1:\d+/g, "HOST") .replace (/HOST\/(?:x_ite\/src|tests\/browser)\//g, "HOST/PAGE/") .replace (/(?:__x_ite_r_web_runtime_02__|runtime02)\.html/g, "page.html"));

// A data URL for the X3D (XML) text.
const xml = text => `data:model/x3d+xml,${encodeURIComponent (text) .replace (/'/g, "%27")}`;

// The page: a canvas with `html`, and a capture listener on the <X3D> element for these events (a CustomEvent does not
// bubble, the capture phase of the ancestors sees it).
const START = ([ html, names ]) =>
{
   const canvas = document .querySelector ("x3d-canvas");

   window .events = [ ];

   const record = event =>
   {
      const target = event .target;
      const node   = event .detail ?.node;
      const value  = event .detail ?.value;

      window .events .push ([
         event .type,
         target .nodeName,
         target .getAttribute ?.("DEF") ?? target .getAttribute ?.("def") ?? "",
         event .constructor .name,
         node ?.getNodeTypeName ?.() ?? null,
         node ?.getNodeName ?.() ?? null,
         value === undefined ? "undefined" : (typeof value === "object" && value !== null ? String (value) : value),
         Object .keys (event .detail ?? { }) .join (),
      ]);
   };

   canvas .innerHTML = html;

   const root = canvas .querySelector ("x3d");

   for (const name of names)
      root .addEventListener (name, record, true);
};

const STATE = () =>
{
   const browser = document .querySelector ("x3d-canvas") .browser;
   const scene   = browser .currentScene;
   const root    = document .querySelector ("x3d-canvas > x3d");

   return {
      vrml: scene .toVRMLString (),
      events: window .events .splice (0),
      dom: root ? root .outerHTML .replace (/\s+/g, " ") : null,
      read: (() =>
      {
         const attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : String (v); } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };

         return {
            roots: attempt (() => Array .from (scene .rootNodes, n => n ?.getNodeTypeName () ?? null)),
            named: attempt (() => Array .from (scene .namedNodes, n => n .getNodeName ())),
            routes: attempt (() => scene .routes .length),
            a: attempt (() => scene .getNamedNode ("Sc") .a),
            b: attempt (() => scene .getNamedNode ("Sc") .b),
         };
      }) (),
   };
};

async function run (page, html, names, steps)
{
   await page .evaluate (START, [ html, names ]);
   await settle (page, 700);

   const result = [ [ "start", await page .evaluate (STATE) ] ];

   for (const [ label, change ] of steps)
   {
      const error = await page .evaluate (change => { try { new Function ("x3d", "document", change) (document .querySelector ("x3d-canvas > x3d"), document); return null; } catch (error) { return String (error); } }, change);

      await settle (page, 500);
      result .push ([ label, error, await page .evaluate (STATE) ]);
   }

   return { steps: result, log: logLines (page) };
}

// The output fields send CustomEvents: named like the field, with the node and the value.
function events (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<Transform DEF='A'><Shape DEF='S'><Box DEF='B'></Box></Shape></Transform>
<Transform DEF='C'></Transform>
<ROUTE fromNode='A' fromField='translation_changed' toNode='C' toField='set_translation'></ROUTE>
</Scene></X3D>`, [ "translation", "scale", "children", "bboxSize", "metadata", "rotation", "geometry", "size", "load", "error" ], [
      [ "attribute", "x3d .querySelector ('[def=A]') .setAttribute ('translation', '1 2 3')" ],
      [ "second", "x3d .querySelector ('[def=A]') .setAttribute ('scale', '2 2 2')" ],
      [ "inputOnly", "x3d .querySelector ('[def=A]') .setAttribute ('bboxSize', '1 1 1')" ],
      [ "added", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<Transform DEF=\"D\" translation=\"4 5 6\"></Transform><ROUTE fromNode=\"A\" fromField=\"translation_changed\" toNode=\"D\" toField=\"set_translation\"></ROUTE>')" ],
      [ "afterAdd", "x3d .querySelector ('[def=A]') .setAttribute ('translation', '7 8 9')" ],
      [ "child", "x3d .querySelector ('[def=A]') .insertAdjacentHTML ('beforeend', '<Group DEF=\"G\"></Group>')" ],
      [ "removed", "x3d .querySelector ('[def=D]') .remove ()" ],
      [ "afterRemove", "x3d .querySelector ('[def=A]') .setAttribute ('translation', '0 0 1')" ],
   ]);
}

const INNER = "<X3D profile='Full'><Scene><Transform DEF='In' translation='1 1 1'><Shape><Box/></Shape></Transform></Scene></X3D>";

// Inline elements: the scene becomes the child element; load and error events.
function inline (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<Inline DEF='I1' url='"${xml (INNER)}"'></Inline>
<Inline DEF='I2' url='"data:model/x3d+xml,nothing"'></Inline>
<Inline DEF='I3' load='false' url='"${xml (INNER)}"'></Inline>
<Group DEF='G'></Group>
</Scene></X3D>`, [ "load", "error", "translation", "children", "loadState" ], [
      [ "inner attribute", "x3d .querySelector ('[def=I1] x3d [DEF=In]') .setAttribute ('translation', '2 2 2')" ],
      [ "load", "x3d .querySelector ('[def=I3]') .setAttribute ('load', 'true')" ],
      [ "unload", "x3d .querySelector ('[def=I1]') .setAttribute ('load', 'false')" ],
      [ "reload", "x3d .querySelector ('[def=I1]') .setAttribute ('load', 'true')" ],
      [ "newUrl", "x3d .querySelector ('[def=I1]') .setAttribute ('url', '\"data:model/x3d+xml,nothing\"')" ],
      [ "addInline", "x3d .querySelector ('scene') .insertAdjacentHTML ('beforeend', '<Inline DEF=\"I4\" url=\\'\"" + xml (INNER) + "\"\\'></Inline>')" ],
      [ "addToInner", "x3d .querySelector ('[def=I4] scene') .insertAdjacentHTML ('beforeend', '<Group DEF=\"Added\"></Group>')" ],
      [ "removeInline", "x3d .querySelector ('[def=I4]') .remove ()" ],
   ]);
}

const TEXT = "data:model/vrml," + encodeURIComponent ("#VRML V2.0 utf8\nGroup { children DEF T Transform { } }") .replace (/'/g, "%27");

// An Inline whose file is not XML: X_ITE leaves the scene's X3D element unset, so the element gets no child (and an XML one
// replaces it, a text one removes it again).
function inlineText (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<Inline DEF='I1' url='"${TEXT}"'></Inline>
</Scene></X3D>`, [ "load", "error", "children", "loadState" ], [
      [ "reload", "x3d .querySelector ('[def=I1]') .setAttribute ('load', 'false'); x3d .querySelector ('[def=I1]') .setAttribute ('load', 'true')" ],
      [ "toXml", "x3d .querySelector ('[def=I1]') .setAttribute ('url', '\"" + xml (INNER) + "\"')" ],
      [ "toText", "x3d .querySelector ('[def=I1]') .setAttribute ('url', '\"" + TEXT + "\"')" ],
   ]);
}

// A removed Scene element: X_ITE disposes the scene (node.dispose of the element's [_node], the X3DScene).
function removeScene (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<Transform DEF='A'><Shape DEF='S'><Box></Box></Shape></Transform>
<Group DEF='G'></Group>
<ROUTE fromNode='A' fromField='translation_changed' toNode='G' toField='set_bboxCenter'></ROUTE>
</Scene></X3D>`, [ "translation", "children", "load" ], [
      [ "remove", "x3d .querySelector ('scene') .remove ()" ],
      [ "afterRemove", "x3d .insertAdjacentHTML ('beforeend', '<Scene><Group DEF=\"N\"></Group></Scene>')" ],
      [ "addToX3D", "x3d .insertAdjacentHTML ('beforeend', '<Group DEF=\"M\"></Group>')" ],
   ]);
}

// A changed attribute of the field element of a user-defined field: the node's field object is the new one.
function userField (page)
{
   return run (page, `<X3D profile='Full'><Scene>
<Script DEF='Sc'><field name='a' type='SFInt32' accessType='inputOutput' value='3'></field></Script>
</Scene></X3D>`, [ "a" ], [
      [ "value", "x3d .querySelector ('field') .setAttribute ('value', '9')" ],
      [ "type", "x3d .querySelector ('field') .setAttribute ('type', 'SFString'); x3d .querySelector ('field') .setAttribute ('value', 'x')" ],
      [ "newField", "x3d .querySelector ('script') .insertAdjacentHTML ('beforeend', '<field name=\"b\" type=\"SFFloat\" accessType=\"inputOutput\" value=\"1.5\"></field>')" ],
   ]);
}

export const CASE_GROUPS = {
   domEvents: { context: DESKTOP, cases: { events, inline, inlineText, removeScene, userField } },
};
