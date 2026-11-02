// The DOM integration 01 cases (issue #146): X3D content written as HTML inside <x3d-canvas>. X_ITE's DOMIntegration reads the
// last `<X3D>` child of the canvas, shows it as the world, and (later cases) keeps it live. oracle/x_ite-web-runtime-02.mjs
// (--suite dom01) runs them against pinned X_ITE 16.5.0 and records the values; dom01.test.mjs runs them against X_ITE_R and
// compares (runtime02_harness.mjs). The worlds are synthetic. The markup is real HTML, so its quirks (a `/>` does not close an
// element, `head` and `meta` in a body) are the browser's, the same for both engines.

import { DESKTOP, settle } from "./runtime02_case.mjs";

const logLines = page => page .log .filter (line => !/^(log|info|debug):/ .test (line)) .map (line => line .replace (/127\.0\.0\.1:\d+/g, "HOST") .replace (/HOST\/(?:x_ite\/src|tests\/browser)\//g, "HOST/PAGE/") .replace (/(?:__x_ite_r_web_runtime_02__|runtime02)\.html/g, "page.html"));

// Put `html` into the canvas and wait for the world.
async function mount (page, html, ms = 600)
{
   await page .evaluate (html => { document .querySelector ("x3d-canvas") .innerHTML = html; }, html);
   await settle (page, ms);
}

const WORLD = `<X3D profile='Full' version='4.0'>
<head><meta name='title' content='dom world'></head>
<Scene>
<Transform DEF='T' translation='1 2 3'><Shape><Appearance><Material diffuseColor='1 0 0'></Material></Appearance><Box size='2 2 2'></Box></Shape></Transform>
<Group DEF='G'><Sphere radius='0.5'></Sphere></Group>
</Scene>
</X3D>`;

// What the page sees of the world: the scene text and the facts of a few nodes.
const READ = () =>
{
   const browser = document .querySelector ("x3d-canvas") .browser;
   const scene   = browser .currentScene;
   const attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : v; } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };

   return {
      vrml: attempt (() => scene .toVRMLString ()),
      profile: attempt (() => scene .profile ?.name),
      components: attempt (() => Array .from (scene .components, c => `${c .name} ${c .level}`)),
      roots: attempt (() => Array .from (scene .rootNodes, n => n ?.getNodeTypeName () ?? null)),
      named: attempt (() => Array .from (scene .namedNodes, n => n .getNodeName ())),
      pageWorld: attempt (() => scene .worldURL === location .href),
   };
};

// A world in the canvas.
async function inlineWorld (page)
{
   await mount (page, WORLD);

   const out = await page .evaluate (READ);

   out .log = logLines (page);

   return out;
}

// Mixed case of node names, field names and the structural elements.
async function names (page)
{
   await mount (page, `<X3D PROFILE='Immersive'><SCENE>
<TRANSFORM def='U' TRANSLATION='1 0 0'><group><SHAPE><box SIZE='1 2 3'></box></SHAPE></group></TRANSFORM>
<Transform USE='U'></Transform>
<WorldInfo title='t' info='"a" "b"'></WorldInfo>
</SCENE></X3D>`);

   const out = await page .evaluate (READ);

   out .log = logLines (page);

   return out;
}


// head statements, several Scene elements, the profile and the version.
async function structure (page)
{
   await page .evaluate (() =>
   {
      const canvas = document .querySelector ("x3d-canvas");
      const x3d    = document .createElement ("X3D");
      const make   = (parent, name, attributes = { }) =>
      {
         const element = document .createElement (name);

         for (const [ key, value ] of Object .entries (attributes))
            element .setAttribute (key, value);

         parent .append (element);

         return element;
      };

      x3d .setAttribute ("profile", "Interchange");
      x3d .setAttribute ("version", "3.3");

      const head = make (x3d, "head");

      make (head, "component", { name: "Geometry3D", level: "2" });
      make (head, "unit", { category: "length", name: "feet", conversionFactor: "0.3048" });
      make (head, "meta", { name: "title", content: "head one" });
      make (head, "meta", { name: "missing" });
      make (head, "component", { name: "Nope", level: "1" });
      make (x3d, "meta", { name: "creator", content: "direct child" });

      const one = make (x3d, "Scene");
      const two = make (x3d, "SCENE");

      make (one, "WorldInfo", { title: "first" });
      make (two, "Group", { DEF: "second" });
      make (x3d, "Group", { DEF: "outside" });
      canvas .append (x3d);
   });
   await settle (page, 600);

   const out = await page .evaluate (READ);

   out .log = logLines (page);

   return out;
}

// ProtoDeclare, ExternProtoDeclare, ProtoInstance, fieldValue, IS and an instance written as an element, in upper and lower case.
async function protos (page)
{
   await mount (page, `<X3D profile='Full'><Scene>
<ProtoDeclare name='Pair' appinfo='info'>
<ProtoInterface><field accessType='inputOutput' type='SFFloat' name='Amount' value='1.5'></field><field accessType='inputOutput' type='SFString' name='label' value='x'></field></ProtoInterface>
<ProtoBody><Transform DEF='Inner'><Shape><Appearance><Material transparency='0.25'><IS><connect nodeField='transparency' protoField='Amount'></connect></IS></Material></Appearance></Shape></Transform></ProtoBody>
</ProtoDeclare>
<PROTODECLARE name='Upper'><PROTOINTERFACE><FIELD accessType='initializeOnly' type='SFBool' name='on' value='true'></FIELD></PROTOINTERFACE><PROTOBODY><Group></Group></PROTOBODY></PROTODECLARE>
<ExternProtoDeclare name='Far' url='"nope.wrl#Far"'><field accessType='inputOutput' type='SFFloat' name='size'></field></ExternProtoDeclare>
<ProtoInstance name='Pair' DEF='I1'><fieldValue name='Amount' value='0.5'></fieldValue></ProtoInstance>
<PAIR def='I2' amount='0.75' LABEL='y'></PAIR>
<pair DEF='I3'></pair>
<UPPER></UPPER>
<Upper on='false'></Upper>
<ProtoInstance name='Far'></ProtoInstance>
<ProtoInstance name='Unknown'></ProtoInstance>
<ProtoInstance name='pair'></ProtoInstance>
<Late></Late>
<ProtoDeclare name='Late'><ProtoInterface></ProtoInterface><ProtoBody><Group></Group></ProtoBody></ProtoDeclare>
<ProtoDeclare name='Again'><ProtoBody><Again2></Again2></ProtoBody></ProtoDeclare>
<ProtoDeclare name='Pair'><ProtoBody><Group></Group></ProtoBody></ProtoDeclare>
</Scene></X3D>`);

   const out = await page .evaluate (READ);

   out .protos = await page .evaluate (() => Array .from (document .querySelector ("x3d-canvas") .browser .currentScene .protos, p => p .name));
   out .externprotos = await page .evaluate (() => Array .from (document .querySelector ("x3d-canvas") .browser .currentScene .externprotos, p => p .name));
   out .log = logLines (page);

   return out;
}

// Script nodes: the source in CDATA, field elements written as text, an invalid one, and what happens to the element.
async function script (page)
{
   // X_ITE_R runs the Scripts of a world only when the canvas lets them; X_ITE ignores the attribute.
   await page .evaluate (() => document .querySelector ("x3d-canvas") .setAttribute ("scripts", "true"));
   await mount (page, `<X3D profile='Full'><Scene>
<Script DEF='S' directOutput='true'><field name='go' type='SFBool' accessType='inputOnly'/><field name='out' type='SFInt32' accessType='outputOnly'/><![CDATA[
ecmascript:
function go (value) { out = value ? 1 : 0; }
]]></Script>
<Script DEF='Bad'>function f () { return 1 < 2; }</Script>
<Group DEF='After'></Group>
</Scene></X3D>`);

   const out = await page .evaluate (() =>
   {
      const result = { };
      const root   = document .querySelector ("x3d-canvas > x3d");
      const scene  = document .querySelector ("x3d-canvas") .browser .currentScene;
      const attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : v; } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };

      result .named = attempt (() => Array .from (scene .namedNodes, n => n .getNodeName ()));
      result .roots = attempt (() => Array .from (scene .rootNodes, n => n .getNodeTypeName ()));
      result .fields = attempt (() => Array .from (scene .getNamedNode ("S") .getFieldDefinitions (), d => [ d .name, d .accessType, d .dataType ]));
      result .vrml = attempt (() => scene .toVRMLString ());
      result .elements = Array .from (root .querySelectorAll ("script"), element =>
         Array .from (element .childNodes, n => [ n .nodeName, n .nodeType, String (n .data ?? "") .slice (0, 60) ]));

      return result;
   });

   out .log = logLines (page);

   return out;
}

// ROUTE, DEF and USE, unknown names, unknown attributes, containerField and a node that is not allowed where it is.
async function wiring (page)
{
   await mount (page, `<X3D profile='Full'><Scene>
<TimeSensor DEF='Clock' cycleInterval='4' enabled='false' CLASS='x' id='ignored' style='color:red'></TimeSensor>
<PositionInterpolator DEF='Move' key='0 1' keyValue='0 0 0, 1 1 1'></PositionInterpolator>
<Transform DEF='Box'><Shape><Box></Box><Appearance containerField='appearance'><Material></Material></Appearance></Shape></Transform>
<Transform DEF='Other'><Shape><Material containerField='geometry'></Material></Shape></Transform>
<ROUTE fromNode='Clock' fromField='fraction_changed' toNode='Move' toField='set_fraction'></ROUTE>
<ROUTE fromnode='Move' fromfield='value_changed' tonode='Box' tofield='set_translation'></ROUTE>
<Transform USE='Box'></Transform>
<Group USE='Box'></Group>
<Group USE='Nowhere'></Group>
<Nothing></Nothing>
<NULL></NULL>
<Group DEF=''></Group>
<Group><Shape USE='Box'></Shape></Group>
</Scene></X3D>`);

   const out = await page .evaluate (READ);

   out .routes = await page .evaluate (() => Array .from (document .querySelector ("x3d-canvas") .browser .currentScene .routes, r => `${r .sourceNode .getNodeName ()}.${r .sourceField} -> ${r .destinationNode .getNodeName ()}.${r .destinationField}`));
   out .log = logLines (page);

   return out;
}

// The world follows the canvas: a second X3D element, the last one, a removed one, a canvas without one, and the browser events.
async function replacing (page)
{
   await page .evaluate (() =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const X3D     = window .X3D_NS;

      window .events = [ ];

      for (const name of [ "INITIALIZED_EVENT", "SHUTDOWN_EVENT", "CONNECTION_ERROR", "BROWSER_EVENT" ])
      {
         if (X3D .X3DConstants [name] !== undefined)
            browser .addBrowserCallback (`events-${name}`, X3D .X3DConstants [name], () => window .events .push (name));
      }
   });

   const steps = [ ];
   const step  = async (label, fn, ms = 500) =>
   {
      await page .evaluate (fn);
      await settle (page, ms);
      steps .push ([ label, await page .evaluate (() =>
      {
         const scene = document .querySelector ("x3d-canvas") .browser .currentScene;

         return [ Array .from (scene .rootNodes, n => n .getNodeTypeName ()), Array .from (scene .namedNodes, n => n .getNodeName ()), window .events .slice () ];
      }) ]);
   };

   const first = "<X3D profile='Full'><Scene><Group DEF='One'></Group></Scene></X3D>";

   await step ("empty", () => { });
   await step ("first", () => { document .querySelector ("x3d-canvas") .insertAdjacentHTML ("beforeend", "<X3D profile='Full'><Scene><Group DEF='One'></Group></Scene></X3D>"); });
   await step ("second", () => { document .querySelector ("x3d-canvas") .insertAdjacentHTML ("beforeend", "<X3D profile='Full'><Scene><Transform DEF='Two'></Transform></Scene></X3D>"); });
   await step ("text", () => { document .querySelector ("x3d-canvas") .append ("text", document .createElement ("div")); });
   await step ("removeLast", () => { document .querySelector ("x3d-canvas > x3d:last-of-type") .remove (); });
   await step ("removeAll", () => { document .querySelector ("x3d-canvas") .replaceChildren (); });
   await step ("again", () => { document .querySelector ("x3d-canvas") .innerHTML = "<X3D profile='Full'><Scene><WorldInfo DEF='Three'></WorldInfo></Scene></X3D>"; });
   await step ("sameElementMoved", () => { const canvas = document .querySelector ("x3d-canvas"); canvas .append (canvas .querySelector ("x3d")); });
   await step ("nested", () => { document .querySelector ("x3d-canvas") .innerHTML = "<div><X3D profile='Full'><Scene><Group DEF='Deep'></Group></Scene></X3D></div>"; });

   return { steps, log: logLines (page) };
}

// browser.importDocument with HTML elements.
async function importElement (page)
{
   await page .evaluate (pageAttempt);

   const out = await page .evaluate (async () =>
   {
      const browser = document .querySelector ("x3d-canvas") .browser;
      const attempt = window .attempt;
      const holder  = document .createElement ("div");
      const result  = { };
      const read    = scene => scene === undefined ? "undefined" : [ scene ?.constructor ?.name, Array .from (scene .rootNodes, n => n .getNodeTypeName ()), scene .profile ?.name, scene .toVRMLString () ];

      holder .innerHTML = "<X3D profile='Immersive'><Scene><TRANSFORM translation='1 2 3'><SHAPE><BOX></BOX></SHAPE></TRANSFORM></Scene></X3D><section><Group></Group></section>";

      result .x3d = read (await browser .importDocument (holder .querySelector ("x3d")));
      result .section = read (await browser .importDocument (holder .querySelector ("section")));
      result .detached = read (await browser .importDocument (holder .firstElementChild));
      result .current = Array .from (browser .currentScene .rootNodes, n => n .getNodeTypeName ());
      result .nothing = await browser .importDocument () .then (() => "resolved", error => `${error ?.constructor ?.name}: ${error ?.message}`);

      return result;
   });

   out .log = logLines (page);

   return out;
}

const pageAttempt = () => { window .attempt = fn => { try { const v = fn (); return v === undefined ? "undefined" : v; } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } }; };


// Field values in attributes: entities, quotes, line breaks, arrays, rotations, nodes in a field.
async function values (page)
{
   await mount (page, `<X3D profile='Full'><Scene>
<WorldInfo title='a &amp; b "c" &lt;d&gt;' info='"one" "two
three" "four &quot;q&quot;"'></WorldInfo>
<Transform rotation='0 1 0 1.5708' scale='1 2 3' center='0 0 0' bboxSize='-1 -1 -1'>
<Shape><IndexedFaceSet coordIndex='0 1 2 -1' solid='false'><Coordinate point='0 0 0
 1 0 0
 0 1 0'></Coordinate></IndexedFaceSet></Shape></Transform>
<Viewpoint DEF='V' position='0 0 8' description='main' orientation='0 0 1 0'></Viewpoint>
<DirectionalLight direction='0 -1 0' intensity='0.5' color='1 0.5 0'></DirectionalLight>
<NavigationInfo type='"EXAMINE" "ANY"' speed='2'></NavigationInfo>
<Background skyColor='0 0 1, 1 1 1' skyAngle='1.5'></Background>
<Script DEF='F'><field name='v' type='MFNode' accessType='initializeOnly'><Group DEF='InField'></Group></field></Script>
<ProtoInstance name='Missing' DEF='M'><fieldValue name='x' value='1'></fieldValue></ProtoInstance>
</Scene></X3D>`);

   const out = await page .evaluate (READ);

   out .log = logLines (page);

   return out;
}

// A profile that does not exist, a version, no Scene, a Scene with text, and markup outside of Scene and head.
async function envelope (page)
{
   const worlds = [
      "<X3D profile='Nope'><Scene><Group DEF='A'></Group></Scene></X3D>",
      "<X3D version='3.0' profile='Interchange'><Scene><Group DEF='B'></Group></Scene></X3D>",
      "<X3D></X3D>",
      "<X3D profile='Core'>text<Scene>more<Group DEF='C'></Group></Scene><Group DEF='Outside'></Group></X3D>",
      "<X3D PROFILE='Immersive'><SCENE><Group DEF='D'></Group></SCENE></X3D>",
   ];
   const result = [ ];

   for (const world of worlds)
   {
      await mount (page, world, 400);
      result .push (await page .evaluate (READ));
   }

   return { result, log: logLines (page) };
}

export const CASE_GROUPS = {
   domImport: { context: DESKTOP, cases: { inlineWorld, names, structure, protos, script, wiring, values, envelope, replacing, importElement } },
};
