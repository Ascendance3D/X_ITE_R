// The Script life cycle 01 cases (issue #133): the browser build's Script nodes against the Script life cycle of the
// SAI and VRML97, with newly authored synthetic worlds. The cases cover a Script's `initialize`, the events of its
// fields (eventIns, eventOuts, routes, page events), `eventsProcessed`, `shutdown`, its errors, and the root nodes
// (`Browser .currentScene` in a Script, and the `rootNodes` field callbacks of the page and of the Script).
// oracle/x_ite-web-runtime-02.mjs (--suite scriptlife01) runs them against pinned X_ITE 16.5.0 and records the values;
// scriptlife01.test.mjs runs them against X_ITE_R (with `<x3d-canvas scripts="true">`) and compares
// (runtime02_harness.mjs).
//
// Each case installs one ordered list, `seq`, before the world loads: every console message (print, warn, error) and
// every callback of the page, in the order they happen, each as one line. What the cases do not record is a time or a
// frame count: the pages of the engines count frames differently.

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { DESKTOP } from "./runtime02_case.mjs";

const WORLDS = {
   // Two Scripts that send events in `initialize`, a route between them, and a Script that changes a node of the scene.
   initialize: `#VRML V2.0 utf8
DEF T Transform { translation 1 2 3 }
DEF A Script
{
  eventOut SFInt32 out
  field SFInt32 n 5
  field MFString names [ "a" "b" "c" ]
  field SFVec3f v 1 2 3
  url "javascript:
function initialize () {
  print ('A initialize n=' + n + ' names=' + names .length + ' v=' + v);
  out = n * 2;
  n = 6;
  names [1] = 'B';
}
function shutdown () { print ('A shutdown n=' + n + ' names=' + names); }
"
}
DEF B Script
{
  eventIn SFInt32 set_in
  field SFInt32 got -1
  url "javascript:
function initialize () { print ('B initialize got=' + got); }
function set_in (value, time) { got = value; print ('B set_in ' + value + ' ' + typeof time); }
function eventsProcessed () { print ('B eventsProcessed got=' + got); }
"
}
DEF C Script
{
  field SFNode target USE T
  directOutput TRUE
  url "javascript:
function initialize () {
  print ('C initialize ' + target .translation);
  target .translation = new SFVec3f (4, 5, 6);
  print ('C initialize ' + target .translation);
}
"
}
ROUTE A.out TO B.set_in
`,

   // Page events and routed events into a Script: eventIn handlers, eventOuts, a route chain, an interpolator, an event of
   // a field the Script reads, the same event sent twice, and values of several field types.
   events: `#VRML V2.0 utf8
DEF I PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0 10 20 30 ] }
DEF S Script
{
  eventIn SFInt32 set_x
  eventIn SFVec3f set_v
  eventIn MFString set_m
  eventIn SFBool set_b
  eventOut SFInt32 x_changed
  eventOut SFVec3f v_changed
  eventOut MFString m_changed
  field SFInt32 x 0
  field SFVec3f v 0 0 0
  url "javascript:
function initialize () { print ('S initialize'); }
function set_x (value, time) {
  print ('S set_x ' + value + ' ' + (time > 0));
  x = value;
  x_changed = value * 10;
  if (value > 100) x_changed = value + 1;
}
function set_v (value) {
  print ('S set_v ' + value);
  v = value;
  v_changed = new SFVec3f (value .z, value .y, value .x);
}
function set_m (value) {
  print ('S set_m ' + value .length + ' ' + value);
  m_changed = new MFString (value [0], 'extra');
}
function set_b (value) { print ('S set_b ' + value); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  eventIn SFVec3f set_w
  eventIn MFString set_n
  eventOut SFInt32 y_changed
  url "javascript:
function set_y (value) { print ('R set_y ' + value); y_changed = value + 1; }
function set_w (value) { print ('R set_w ' + value); }
function set_n (value) { print ('R set_n ' + value); }
"
}
DEF Q Script
{
  eventIn SFInt32 set_z
  url "javascript: function set_z (value) { print ('Q set_z ' + value); }"
}
ROUTE I.value_changed TO S.set_v
ROUTE S.x_changed TO R.set_y
ROUTE S.v_changed TO R.set_w
ROUTE S.m_changed TO R.set_n
ROUTE R.y_changed TO Q.set_z
`,

   // eventsProcessed: once after the events of a batch, never without an event, and it can send events.
   eventsProcessed: `#VRML V2.0 utf8
DEF A Script
{
  eventIn SFInt32 set_a
  eventIn SFInt32 set_b
  eventOut SFInt32 sum
  field SFInt32 a 0
  field SFInt32 b 0
  url "javascript:
function set_a (value) { a = value; print ('A set_a ' + value); }
function set_b (value) { b = value; print ('A set_b ' + value); }
function eventsProcessed () { print ('A eventsProcessed ' + a + ' ' + b); sum = a + b; }
"
}
DEF B Script
{
  eventIn SFInt32 set_sum
  url "javascript:
function set_sum (value) { print ('B set_sum ' + value); }
function eventsProcessed () { print ('B eventsProcessed'); }
"
}
DEF C Script
{
  eventIn SFInt32 set_c
  url "javascript: function set_c (value) { print ('C set_c ' + value); } function eventsProcessed () { print ('C eventsProcessed'); }"
}
ROUTE A.sum TO B.set_sum
`,

   // shutdown: when the world is replaced, once, the order of the Scripts, an event sent from it, and the field values.
   shutdown: `#VRML V2.0 utf8
DEF A Script
{
  eventOut SFInt32 out
  field SFInt32 n 1
  url "javascript:
function initialize () { print ('A initialize'); }
function set_n (value) { n = value; }
function shutdown () { print ('A shutdown n=' + n); out = 99; }
"
  eventIn SFInt32 set_n
}
DEF B Script
{
  eventIn SFInt32 set_in
  url "javascript:
function initialize () { print ('B initialize'); }
function set_in (value) { print ('B set_in ' + value); }
function shutdown () { print ('B shutdown'); }
"
}
Script { url "javascript: function shutdown () { print ('C shutdown'); }" }
ROUTE A.out TO B.set_in
`,

   // Errors: in initialize, in an event handler, in eventsProcessed, in shutdown, at the top level of the code, a
   // syntax error, a wrong event value, and a thrown value that is no Error. The next Script and the next event go on.
   errors: `#VRML V2.0 utf8
DEF A Script
{
  eventIn SFInt32 set_x
  url "javascript:
function initialize () { print ('A initialize'); throw new Error ('boom in initialize'); }
function set_x (value) { print ('A set_x ' + value); if (value == 2) throw new Error ('boom in set_x'); print ('A set_x done ' + value); }
function eventsProcessed () { print ('A eventsProcessed'); undefinedFunction (); }
function shutdown () { print ('A shutdown'); throw new TypeError ('boom in shutdown'); }
"
}
DEF E Script
{
  eventIn SFInt32 set_x
  url "javascript:
print ('E top level');
nothingHere .property;
function initialize () { print ('E initialize'); }
function set_x (value) { print ('E set_x ' + value); }
"
}
DEF B Script
{
  eventIn SFInt32 set_x
  eventOut SFColor c_changed
  url "javascript:
print ('B top level');
function initialize () { print ('B initialize'); }
function set_x (value) { print ('B set_x ' + value); if (value == 3) c_changed = new SFVec3f (1, 2, 3); if (value == 4) throw 'a string'; if (value == 5) throw { a: 1 }; }
"
}
DEF C Script
{
  eventIn SFInt32 set_x
  url "javascript: function initialize () { print ('C initialize'); } function set_x (value) { print ('C set_x ' + value); } function (  { "
}
DEF D Script
{
  eventIn SFInt32 set_x
  field SFInt32 n 0
  url "javascript:
function initialize () { print ('D initialize'); }
function set_x (value) { print ('D set_x ' + value); n = 'not a number'; print ('D n=' + n + ' ' + typeof n); n = new SFVec3f (); }
"
}
`,

   // The root nodes: a Script that reads and changes them on events, with a field callback of its own (the callback is
   // added in `initialize`), and a Group X to remove. Each case changes the root nodes in one way.
   rootNodes: `#VRML V2.0 utf8
DEF A Transform { }
DEF X Group { }
DEF H Group { children DEF S Script
{
  eventIn SFTime add
  eventIn SFTime remove
  eventIn SFTime set
  eventIn SFTime push
  eventIn SFTime read
  field SFNode saved NULL
  url "javascript:
function names (nodes) { var r = []; for (var i = 0; i < nodes .length; ++ i) r .push (nodes [i] ? nodes [i] .getNodeTypeName () + ':' + nodes [i] .getNodeName () : 'NULL'); return r .join (','); }
function initialize () {
  var scene = Browser .currentScene;
  print ('S initialize ' + names (scene .rootNodes));
  scene .rootNodes .addFieldCallback ('script', function (value) { print ('S callback ' + value .length + ' ' + names (value)); });
}
function add () {
  var scene = Browser .currentScene;
  saved = scene .createNode ('Group');
  scene .addRootNode (saved);
  print ('S add ' + names (scene .rootNodes));
}
function remove () {
  var scene = Browser .currentScene;
  scene .removeRootNode (scene .getNamedNode ('X'));
  print ('S remove ' + names (scene .rootNodes));
}
function set () {
  var scene = Browser .currentScene;
  scene .setRootNodes (new MFNode (scene .createNode ('Transform'), scene .getNamedNode ('X')));
  print ('S set ' + names (scene .rootNodes));
}
function push () {
  var scene = Browser .currentScene;
  scene .rootNodes .push (scene .createNode ('Group'));
  print ('S push ' + names (scene .rootNodes));
}
function read () { print ('S read ' + names (Browser .currentScene .rootNodes)); }
"
} }
`,
   // Item 1 of issue #202: the same Script as XML and as X3D JSON, for importDocument and importJS.
   importXml: `<X3D profile='Full' version='4.0'><Scene>
<Transform DEF='T' translation='1 2 3'/>
<Script DEF='S' directOutput='true'>
<field name='set_x' type='SFInt32' accessType='inputOnly'/>
<field name='x_changed' type='SFInt32' accessType='outputOnly'/>
<field name='n' type='SFInt32' accessType='initializeOnly' value='5'/>
<field name='target' type='SFNode' accessType='initializeOnly'><Transform USE='T'/></field>
<![CDATA[ecmascript:
function initialize () { print ('S initialize n=' + n + ' ' + target .translation); target .translation = new SFVec3f (4, 5, 6); }
function set_x (value) { n = value; print ('S set_x ' + value); x_changed = value * 2; }
function eventsProcessed () { print ('S eventsProcessed n=' + n); }
function shutdown () { print ('S shutdown'); }
]]></Script>
</Scene></X3D>`,

   importJson: JSON .stringify ({ X3D: { "@profile": "Full", "@version": "4.0", Scene: { "-children": [
      { Transform: { "@DEF": "T", "@translation": [ 1, 2, 3 ] } },
      { Script: { "@DEF": "S", "@directOutput": true,
         field: [
            { "@name": "set_x", "@type": "SFInt32", "@accessType": "inputOnly" },
            { "@name": "x_changed", "@type": "SFInt32", "@accessType": "outputOnly" },
            { "@name": "n", "@type": "SFInt32", "@accessType": "initializeOnly", "@value": 5 },
            { "@name": "target", "@type": "SFNode", "@accessType": "initializeOnly", "-children": [ { Transform: { "@USE": "T" } } ] } ],
         "#sourceText": [
            "ecmascript:",
            "function initialize () { print ('S initialize n=' + n + ' ' + target .translation); target .translation = new SFVec3f (4, 5, 6); }",
            "function set_x (value) { n = value; print ('S set_x ' + value); x_changed = value * 2; }",
            "function eventsProcessed () { print ('S eventsProcessed n=' + n); }",
            "function shutdown () { print ('S shutdown'); }" ] } } ] } } }),

   // Item 2: functions and nodes that a Script leaves in the page, used after its world was replaced.
   staleFunction: `#VRML V2.0 utf8
DEF T Transform { translation 1 2 3 }
DEF S Script
{
  eventIn SFInt32 set_x
  eventOut SFInt32 x_changed
  field SFInt32 n 7
  field SFNode target USE T
  directOutput TRUE
  url "javascript:
function initialize () {
  window .kept = {
    read: function () { return n + ' ' + target .translation; },
    write: function (v) { n = v; target .translation = new SFVec3f (v, v, v); return 'written ' + v; },
    send: function (v) { x_changed = v; return 'sent ' + v; },
    print: function () { print ('kept print n=' + n); return 'printed'; },
    node: target,
    scene: Browser .currentScene
  };
  print ('S initialize');
}
function set_x (value) { n = value; }
"
}
`,

   // Item 3: a Script (and the page) that interleave console output with calls into the page.
   printTiming: `#VRML V2.0 utf8
DEF S Script
{
  eventIn SFInt32 set_x
  eventOut SFInt32 x_changed
  url "javascript:
function initialize () { print ('S initialize 1'); window .mark ('in initialize'); print ('S initialize 2'); }
function set_x (value) {
  print ('S set_x a');
  window .mark ('in set_x 1');
  console .log ('S console.log');
  window .mark ('in set_x 2');
  Browser .print ('S Browser.print', 1, 'two');
  Browser .println ('S Browser.println');
  console .warn ('S console.warn');
  window .mark ('in set_x 3');
  x_changed = value;
  window .mark ('after x_changed');
  print ('S set_x b');
}
function eventsProcessed () { print ('S eventsProcessed'); window .mark ('in eventsProcessed'); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  url "javascript: function set_y (value) { print ('R set_y ' + value); window .mark ('in R set_y'); print ('R set_y done'); }"
}
ROUTE S.x_changed TO R.set_y
`,
};

// The case of a world: load it, run the steps, and answer the ordered `seq` with the values the steps returned.
async function scenario (page, name)
{
   return page .evaluate (async ([ name, world ]) =>
   {
      const X3D     = window .X3D_NS;
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = X3D .getBrowser (canvas);
      const frame   = () => new Promise (r => requestAnimationFrame (() => r ()));
      const spin    = async n => { for (let i = 0; i < n; ++ i) await frame (); };
      const seq     = [ ];
      const out     = { };
      const saved   = Object .fromEntries ([ "log", "warn", "error" ] .map (type => [ type, console [type] ]));
      const errors  = [ ];
      const onError = event => { seq .push (`pageerror: ${String (event .error ?.message ?? event .message)}`); event .preventDefault (); };

      // X_ITE_R's page runs a Script only when the page allows it (the attribute is unknown to X_ITE).
      canvas .setAttribute ("scripts", "true");

      for (const type of Object .keys (saved))
         console [type] = (... args) => seq .push (`${type}: ${args .join (" ") .split ("\n") [0]}`);

      window .addEventListener ("error", onError);

      const attempt = fn => { try { return fn (); } catch (error) { return `${error ?.constructor ?.name}: ${error ?.message}`; } };
      const scene   = () => browser .currentScene;
      const node    = id => scene () .getNamedNode (id);
      // A new node of the scene (X3DField.fromVRMLString with the scene; the page has no scene.createNode).
      const make    = vrml => { const f = new X3D .SFNode (); f .fromVRMLString (vrml, scene ()); return f .valueOf (); };
      const note    = text => seq .push (`page: ${text}`);
      const names   = nodes => Array .from (nodes, n => n ? `${n .getNodeTypeName ()}:${n .getNodeName ()}` : "NULL") .join (",");
      const load    = async text => { await browser .replaceWorld (await browser .createX3DFromString (text)); await spin (4); };
      const imported = async (how, text) =>
      {
         const doc = () => new DOMParser () .parseFromString (text, "application/xml");
         const scene = await ({
            jsString:    () => browser .importJS (text),
            jsObject:    () => browser .importJS (JSON .parse (text)),
            xmlString:   () => browser .importDocument (text),
            xmlDocument: () => browser .importDocument (doc ()),
            xmlElement:  () => browser .importDocument (doc () .documentElement),
         }) [how] ();

         await browser .replaceWorld (scene);
         await spin (4);
      };

      try
      {
         // A function the Script of a case calls to put a line into `seq` from inside its own call.
         window .mark = text => note (`mark ${text}`);

         const how = { importJsString: "jsString", importJsObject: "jsObject", importXmlString: "xmlString", importXmlDocument: "xmlDocument", importXmlElement: "xmlElement" } [name];

         if (how)
         {
            note ("import");
            await imported (how, world);
         }
         else
            await load (world);

         switch (name)
         {
            case "importJsString":
            case "importJsObject":
            case "importXmlString":
            case "importXmlDocument":
            case "importXmlElement":
            {
               const S = node ("S");

               S .addFieldCallback ("page", "x_changed", v => note (`x_changed ${v}`));
               out .fields = [ S .n, String (node ("T") .translation) ];
               note ("set_x 3");
               S .set_x = 3;
               await spin (3);
               out .after = [ S .n, String (node ("T") .translation) ];
               out .vrml  = scene () .toVRMLString () .replace (/\s+/g, " ") .trim () .split (" url ") [0];
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nGroup { }\n");
               await spin (10);
               break;
            }
            case "staleFunction":
            {
               const kept = window .kept;

               out .before = attempt (() => kept .read ());
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nDEF G Group { }\n");
               out .read  = attempt (() => kept .read ());
               out .write = attempt (() => kept .write (9));
               out .send  = attempt (() => kept .send (4));
               out .print = attempt (() => kept .print ());
               out .node  = attempt (() => String (kept .node .translation));
               out .scene = attempt (() => kept .scene .rootNodes .length);
               out .world = names (scene () .rootNodes);
               await spin (5);
               break;
            }
            case "printTiming":
            {
               const S = node ("S");

               S .addFieldCallback ("page", "x_changed", v => { note (`x_changed ${v}`); browser .print ("page print in callback"); console .log ("page console.log in callback"); });
               note ("set_x 1");
               S .set_x = 1;
               note ("after the assignment");
               await spin (3);
               note ("page print");
               browser .print ("page Browser.print");
               browser .println ("page Browser.println");
               note ("after the page print");
               await spin (3);
               break;
            }
            case "initialize":
            {
               out .fields = [ node ("A") .n, Array .from (node ("A") .names) .join (), String (node ("A") .v), node ("B") .got, String (node ("T") .translation) ];
               break;
            }
            case "events":
            {
               const S = node ("S");

               S .addFieldCallback ("page", "x_changed", v => note (`x_changed ${v}`));
               S .addFieldCallback ("page", "v_changed", v => note (`v_changed ${v}`));
               S .addFieldCallback ("page", "m_changed", v => note (`m_changed ${v}`));

               note ("set_x 1");
               S .set_x = 1;
               await spin (3);
               note ("set_x 2 and 3 in one task");
               S .set_x = 2;
               S .set_x = 3;
               await spin (3);
               note ("set_v");
               S .set_v = new X3D .SFVec3f (1, 2, 3);
               await spin (3);
               note ("set_m");
               S .set_m = new X3D .MFString ("p", "q");
               await spin (3);
               note ("set_b");
               S .set_b = true;
               await spin (3);
               note ("set_x 101 (a second event of the Script in one handler)");
               S .set_x = 101;
               await spin (3);
               note ("interpolator");
               node ("I") .set_fraction = 0.5;
               await spin (3);
               out .fields = [ S .x, String (S .v), node ("Q") .set_z ];
               note ("set_x with a string");
               out .wrongType = attempt (() => { S .set_x = "text"; return S .set_x; });
               await spin (3);
               break;
            }
            case "eventsProcessed":
            {
               const A = node ("A");

               note ("nothing");
               await spin (4);
               note ("set_a 1");
               A .set_a = 1;
               await spin (3);
               note ("set_a 2, set_b 3 in one task");
               A .set_a = 2;
               A .set_b = 3;
               await spin (3);
               note ("set_c 4");
               node ("C") .set_c = 4;
               await spin (3);
               note ("set_a 5, then a frame, then set_b 6");
               A .set_a = 5;
               await spin (2);
               A .set_b = 6;
               await spin (3);
               break;
            }
            case "shutdown":
            {
               node ("A") .set_n = 7;
               await spin (3);
               out .before = [ node ("A") .n ];
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nDEF G Group { }\n");
               note ("replaceWorld again");
               await load ("#VRML V2.0 utf8\nDEF H Group { }\n");
               out .after = names (scene () .rootNodes);
               await spin (30);
               break;
            }
            case "shutdownUrl":
            {
               // New code for a running Script: the old code's shutdown, then the new code's initialize.
               note ("url");
               node ("A") .url = new X3D .MFString ("javascript: function initialize () { print ('A2 initialize n=' + n); } function shutdown () { print ('A2 shutdown'); }");
               await spin (10);
               out .n = node ("A") .n;
               break;
            }
            case "shutdownDispose":
            {
               // The element leaves the document: X_ITE disposes the browser and its world.
               note ("dispose");
               canvas .remove ();
               await spin (10);
               break;
            }
            case "errors":
            {
               for (const value of [ 1, 2, 3, 4, 5, 6 ])
               {
                  note (`set_x ${value}`);
                  for (const id of [ "A", "B", "C", "D", "E" ])
                     node (id) .set_x = value;
                  await spin (3);
               }

               out .d = [ node ("D") .n ];
               note ("replaceWorld");
               await load ("#VRML V2.0 utf8\nGroup { }\n");
               break;
            }
            case "rootNodes":
            case "rootNodesPage":
            case "rootNodesAdd":
            case "rootNodesRemove":
            case "rootNodesSet":
            case "rootNodesPush":
            {
               const S = node ("S");
               const how = name .slice ("rootNodes" .length) || "Page";

               scene () .rootNodes .addFieldCallback ("page", v => note (`rootNodes callback ${v .length} ${names (v)}`));
               out .initial = names (scene () .rootNodes);

               if (how === "Page")
               {
                  const G = make ("Group { }");

                  note ("page addRootNode");
                  out .add = attempt (() => scene () .addRootNode (G));
                  await spin (3);
                  note ("page push");
                  out .push = attempt (() => scene () .rootNodes .push (make ("Transform { }")));
                  await spin (3);

                  note ("page removeRootNode");
                  out .remove = attempt (() => scene () .removeRootNode (node ("X")));
                  await spin (3);
                  note ("Script read");
                  S .read = 1;
                  await spin (3);
               }
               else
               {
                  note (`Script ${how .toLowerCase ()}`);
                  S [how .toLowerCase ()] = 1;
                  await spin (3);
                  note ("Script read");
                  S .read = 2;
                  await spin (3);
               }

               out .final = names (scene () .rootNodes);
               out .vrml  = scene () .toVRMLString () .replace (/\s+/g, " ") .trim () .replace (/ url "[^"]*"/, " url <code>");
               break;
            }
         }
      }
      finally
      {
         Object .assign (console, saved);
         window .removeEventListener ("error", onError);
      }

      out .errors = errors;
      out .seq    = seq;

      return JSON .parse (JSON .stringify (out, (key, value) => value === undefined ? null : value));
   },
   [ name, WORLDS [WORLD_OF (name)] ]);
}

// The Script of the root nodes cases is in a Group, not a root node: X_ITE 16.5.0 throws `array[i].set is not a
// function` for a change of the root nodes while a Script is a root node itself, and the cases do not record that.
const WORLD_OF = name => name .startsWith ("shutdown") ? "shutdown" : name .startsWith ("rootNodes") ? "rootNodes" : name .startsWith ("importJs") ? "importJson" : name .startsWith ("importXml") ? "importXml" : name;

const cases = Object .fromEntries ([ ... Object .keys (WORLDS) .filter (name => ![ "rootNodes", "importXml", "importJson" ] .includes (name)), "shutdownUrl", "shutdownDispose", "importJsString", "importJsObject", "importXmlString", "importXmlDocument", "importXmlElement", ... [ "Page", "Add", "Remove", "Set", "Push" ] .map (how => `rootNodes${how}`) ] .map (name => [ name, page => scenario (page, name) ]));

// --- The reload sets (issue #202, item 4) ---------------------------------------------------------------------------------
//
// oracle/synthetic-inline-reload and oracle/synthetic-externproto-reload are step scenarios: each frame of a scenario has
// actions on an Inline (or an EXTERNPROTO), and the native suites compare them frame by frame with the pinned X_ITE's
// stepped page (oracle/x_ite-inline.mjs). A real page cannot step the frames, so these cases drive a scenario with the
// page's API only and compare what is settled: after the world and after each frame that has actions, they wait 400 ms, then
// in windows of 650 ms (longer than the 0.5 s cycle of the worlds' clocks) until a window has a clock tick (at most about 5 s,
// as a slow runner starts the clocks late), and one window more; the ticks of these windows count. They record the console lines that name a state (`warn`, `error`, and
// the `initialize`, `shutdown` and `created` prints), the digit-normalized set of the other lines (the clocks' ticks: only
// whether a clock ran, not how often), and the `url` and `load` of the Inlines the scenario drives.
//
// Only the scenarios that a page can drive in both engines are cases: actions of the types `url`, `load`, `remove` and
// `publish` on a named Inline of the world (X_ITE_R's page has no `loadNow`, `unloadNow`, `autoRefresh` timing in frames
// or EXTERNPROTO declaration objects, and cannot reach the Inlines of an Inline), no two actions in adjacent frames
// (their order against the loads is frame timing), and no scenario that the native suite marks `skip`. Of the EXTERNPROTO
// set, the scenarios without actions.

const REPO         = join (dirname (fileURLToPath (import.meta.url)), "../../../..");
const RELOAD_SETS  = { inline: "synthetic-inline-reload", externproto: "synthetic-externproto-reload" };
const RELOAD_TYPES = [ "url", "load", "remove", "publish" ];
const EARLY_MS     = 400;
const WINDOW_MS    = 650;
// The most windows a step waits for its first clock tick (about 5 s): a slow machine starts the clocks late.
const MAX_WINDOWS  = 8;

function reloadScenarios ()
{
   const found = { };

   for (const [ prefix, set ] of Object .entries (RELOAD_SETS))
   {
      for (const file of readdirSync (join (REPO, "oracle", set)) .filter (f => /^c.*\.json$/ .test (f)) .sort ())
      {
         const scenario = JSON .parse (readFileSync (join (REPO, "oracle", set, file), "utf8"));
         const actions  = scenario .actions ?? [ ];
         const frames   = [ ... new Set (actions .map (a => a .frame)) ] .sort ((a, b) => a - b);

         if (scenario .oracle === "skip")
            continue;

         if (actions .some (a => !RELOAD_TYPES .includes (a .type) || (a .name ?? "") .includes ("/")))
            continue;

         if (set === RELOAD_SETS .externproto && actions .length)
            continue;

         if (frames .some ((f, i) => i && f - frames [i - 1] < 3))
            continue;

         // c35 lives on the Inline's wall-clock autoRefresh: X_ITE retries a failed load again and again, so the number of its error
         // lines grows with the time a step waits (3 in 1 s, 9 in 5 s); the native suites compare it frame by frame.
         if (set === RELOAD_SETS .inline && file === "c35-auto-refresh-after-failure.json")
            continue;

         // The Driver of the EXTERNPROTO worlds moves an instance at its second tick, which a wall clock places in one or the
         // next window: c01 is the one scenario that shows it in the last window of its only step.
         if (set === RELOAD_SETS .externproto && file === "c01-initial.json")
            continue;

         found [`${prefix}.${file .slice (0, -5)}`] = [ set, scenario ];
      }
   }

   return found;
}

async function reloadCase (page, env, set, scenario)
{
   const dir     = join (REPO, "oracle", set);
   const actions = scenario .actions ?? [ ];
   const names   = [ ... new Set ([ ... (set === RELOAD_SETS .inline ? [ "Inl" ] : [ ]), ... actions .map (a => a .name) .filter (Boolean) ]) ];
   const frames  = [ ... new Set (actions .map (a => a .frame)) ] .sort ((a, b) => a - b);
   const steps   = [ ];

   env .server .unpublish ();

   // The lines of the first EARLY_MS after an action (a clock of the replaced scene may still run) are not counted as ticks. Then
   // the step waits in windows of WINDOW_MS until one has a tick (a slow machine starts the clocks late; at most MAX_WINDOWS, for
   // a step in which nothing ticks), and one window more, and the ticks are those of these windows.
   const settle = () => page .evaluate (async ([ early, win, max ]) =>
   {
      const r    = window .__reload;
      const wait = ms => new Promise (resolve => setTimeout (resolve, ms));

      await wait (early);
      r .mark = r .lines .length;

      for (let i = 0; i < max; ++ i)
      {
         await wait (win);

         if (r .lines .slice (r .mark) .some (line => !r .states (line)))
         {
            await wait (win);
            return;
         }

         if (i < max - 1)
            r .mark = r .lines .length;
      }
   }, [ EARLY_MS, WINDOW_MS, MAX_WINDOWS ]);

   const snapshot = async () => page .evaluate (names =>
   {
      const X3D      = window .X3D_NS;
      const browser  = X3D .getBrowser (document .querySelector ("x3d-canvas"));
      const lines    = window .__reload .lines .splice (0);
      const late     = lines .slice (window .__reload .mark);
      const states   = window .__reload .states;
      const inlines  = { };

      for (const name of names)
      {
         try
         {
            const node = browser .currentScene .getNamedNode (name);

            inlines [name] = [ Array .from (node .url) .map (u => u .split (window .location .origin) .join ("http://oracle")), node .load ];
         }
         catch (error) { inlines [name] = String (error ?.message ?? error); }
      }

      return {
         stable: lines .filter (states),
         ticks:  [ ... new Set (late .filter (line => !states (line)) .map (line => line .replace (/\d+(\.\d+)?/g, "N"))) ] .sort (),
         inlines,
      };
   }, names);

   await page .evaluate (async ([ url ]) =>
   {
      const X3D     = window .X3D_NS;
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = X3D .getBrowser (canvas);
      const lines   = [ ];
      const origin  = window .location .origin;

      window .__reload = { lines, mark: 0, states: line => !line .startsWith ("log: ") || /\b(initialize|shutdown|created)\b/ .test (line) };
      canvas .setAttribute ("scripts", "true");

      for (const type of [ "log", "warn", "error" ])
         console [type] = (... args) => lines .push (`${type}: ${args .join (" ") .split ("\n") [0] .split (origin) .join ("http://oracle")}`);

      window .addEventListener ("error", event => { lines .push (`pageerror: ${String (event .error ?.message ?? event .message)}`); event .preventDefault (); });

      try { await browser .loadURL (new X3D .MFString (url)); }
      catch (error) { lines .push (`loadURL rejected: ${String (error ?.message ?? error)}`); }
   }, [ `${env .base}/${set}/${scenario .world}` ]);

   await settle ();
   steps .push ({ k: 0, ... await snapshot () });

   for (const frame of frames)
   {
      const now = actions .filter (a => a .frame === frame);

      for (const a of now .filter (a => a .type === "publish"))
         env .server .publish (`/${set}/${a .path}`, readFileSync (join (dir, a .from)));

      // All the actions of one frame in one task, as the oracle applies them between two frames.
      await page .evaluate (now =>
      {
         const X3D     = window .X3D_NS;
         const browser = X3D .getBrowser (document .querySelector ("x3d-canvas"));
         const node    = name => browser .currentScene .getNamedNode (name);

         for (const a of now)
         {
            switch (a .type)
            {
               case "url":    node (a .name) .url = new X3D .MFString (... a .url); break;
               case "load":   node (a .name) .load = a .value; break;
               case "remove":
               {
                  const group = node (a .group);

                  group .children = new X3D .MFNode (... Array .from (group .children) .filter (c => c ?.getNodeName () !== a .name));
                  break;
               }
            }
         }
      }, now .filter (a => a .type !== "publish"));

      await settle ();
      steps .push ({ k: frame, ... await snapshot () });
   }

   return { steps };
}

const reloadCases = Object .fromEntries (Object .entries (reloadScenarios ()) .map (([ key, [ set, scenario ] ]) => [ key, (page, env) => reloadCase (page, env, set, scenario) ]));

export const CASE_GROUPS = {
   scriptLife: { context: DESKTOP, cases },
   scriptReload: { context: DESKTOP, cases: reloadCases },
};
