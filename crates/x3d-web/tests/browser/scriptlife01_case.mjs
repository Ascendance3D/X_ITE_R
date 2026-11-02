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

   // Issue #202, item 1 (a callback that changes the world inside the route chain S -> R -> Q): the page's callback on
   // S.x_changed adds and removes root nodes (chainRoots) or gives Q new code (chainUrl); R and Q read the world.
   chainWorld: `#VRML V2.0 utf8
DEF T Transform { }
DEF S Script
{
  eventIn SFInt32 set_x
  eventOut SFInt32 x_changed
  url "javascript:
function set_x (value) { print ('S set_x ' + value); x_changed = value; print ('S after x_changed'); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  eventOut SFInt32 y_changed
  url "javascript:
function names (nodes) { var r = []; for (var i = 0; i < nodes .length; ++ i) r .push (nodes [i] ? nodes [i] .getNodeName () : 'NULL'); return r .join (','); }
function set_y (value) { print ('R set_y ' + value + ' roots=' + names (Browser .currentScene .rootNodes)); y_changed = value; }
"
}
DEF Q Script
{
  eventIn SFInt32 set_z
  url "javascript: function initialize () { print ('Q1 initialize'); } function set_z (value) { print ('Q1 set_z ' + value + ' roots=' + Browser .currentScene .rootNodes .length); } function shutdown () { print ('Q1 shutdown'); }"
}
ROUTE S.x_changed TO R.set_y
ROUTE R.y_changed TO Q.set_z
`,

   // Issue #202, item 2 (the output of an element that is disposed): a Script that removes its own <x3d-canvas> in the middle of
   // its call and prints on, a route to a second Script, and a clock whose events print each frame.
   disposeInCall: `#VRML V2.0 utf8
DEF C TimeSensor { loop TRUE cycleInterval 0.05 }
DEF S Script
{
  eventIn SFInt32 set_x
  eventIn SFTime tick
  eventOut SFInt32 x_changed
  field SFInt32 ticks 0
  url "javascript:
function set_x (value) { print ('S before dispose'); window .mark ('S before'); window .removeCanvas (); print ('S after dispose'); window .mark ('S after'); x_changed = value; print ('S end'); }
function tick (time) { ticks = ticks + 1; if (ticks < 4) print ('S tick ' + ticks); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  url "javascript: function set_y (value) { print ('R set_y ' + value); }"
}
ROUTE C.cycleTime TO S.tick
ROUTE S.x_changed TO R.set_y
`,

   // Issue #202, item 3 (the timing of the promise jobs): a handler that starts jobs, a job that sends an event and one that
   // prints from a second job, and an async function.
   promiseJobs: `#VRML V2.0 utf8
DEF S Script
{
  eventIn SFInt32 set_x
  eventIn SFInt32 set_a
  eventOut SFInt32 x_changed
  url "javascript:
function set_x (value) {
  print ('S set_x ' + value);
  Promise .resolve () .then (function () { print ('job 1'); window .mark ('job 1'); x_changed = value + 1; Promise .resolve () .then (function () { print ('job 2'); }); });
  print ('S set_x end');
  window .mark ('S set_x end');
}
async function set_a (value) {
  print ('async begin');
  await null;
  print ('async after await');
  x_changed = value + 10;
}
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  url "javascript: function set_y (value) { print ('R set_y ' + value); }"
}
ROUTE S.x_changed TO R.set_y
`,

   // Issue #202, item 4 (a Script that is a root node): the Script is a root node of the world, and the page changes the root
   // nodes. ISO/IEC 14772-1 allows a Script wherever a node statement is allowed, so the root nodes may hold one.
   rootScript: `#VRML V2.0 utf8
DEF A Transform { }
DEF X Group { }
DEF S Script
{
  eventIn SFTime read
  eventIn SFTime add
  field SFNode saved NULL
  url "javascript:
function names (nodes) { var r = []; for (var i = 0; i < nodes .length; ++ i) r .push (nodes [i] ? nodes [i] .getNodeTypeName () + ':' + nodes [i] .getNodeName () : 'NULL'); return r .join (','); }
function initialize () { print ('S initialize ' + names (Browser .currentScene .rootNodes)); }
function read () { print ('S read ' + names (Browser .currentScene .rootNodes)); }
function add () { Browser .currentScene .addRootNode (Browser .currentScene .createNode ('Group')); print ('S add ' + names (Browser .currentScene .rootNodes)); }
function shutdown () { print ('S shutdown'); }
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

   // Issue #202, item 1: page callbacks on fields inside a route chain S -> R -> Q. The callback on S.x_changed writes another
   // field (T.translation) and reads fields; the callback on R.y_changed reads T.translation.
   callbackInChain: `#VRML V2.0 utf8
DEF T Transform { }
DEF S Script
{
  eventIn SFInt32 set_x
  eventOut SFInt32 x_changed
  url "javascript:
function set_x (value) { window .mark ('S set_x ' + value); x_changed = value; window .mark ('S after x_changed'); }
function eventsProcessed () { window .mark ('S eventsProcessed'); }
"
}
DEF R Script
{
  eventIn SFInt32 set_y
  eventOut SFInt32 y_changed
  field SFNode t USE T
  directOutput TRUE
  url "javascript:
function set_y (value) { window .mark ('R set_y ' + value + ' T=' + t .translation); y_changed = value; window .mark ('R after y_changed'); }
function eventsProcessed () { window .mark ('R eventsProcessed'); }
"
}
DEF Q Script
{
  eventIn SFInt32 set_z
  url "javascript: function set_z (value) { window .mark ('Q set_z ' + value); }"
}
ROUTE S.x_changed TO R.set_y
ROUTE R.y_changed TO Q.set_z
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
            case "callbackInChain":
            {
               const S = node ("S");
               const R = node ("R");
               const T = node ("T");

               S .addFieldCallback ("page", "x_changed", v => { note (`x_changed ${v} T=${T .translation} S.x=${S .x_changed}`); T .translation = new X3D .SFVec3f (v, 0, 0); note (`x_changed wrote T=${T .translation}`); });
               R .addFieldCallback ("page", "y_changed", v => note (`y_changed ${v} T=${T .translation}`));
               T .addFieldCallback ("page", "translation_changed", v => note (`translation_changed ${v}`));
               note ("set_x 1");
               S .set_x = 1;
               note ("after the assignment");
               await spin (3);
               out .final = String (T .translation);
               break;
            }
            case "chainRoots":
            case "chainUrl":
            {
               const S = node ("S");
               const T = node ("T");

               S .addFieldCallback ("page", "x_changed", v =>
               {
                  note (`x_changed ${v}`);

                  if (name === "chainRoots")
                  {
                     out .added   = attempt (() => scene () .addRootNode (make ("DEF N Transform { }")));
                     out .removed = attempt (() => scene () .removeRootNode (T));
                  }
                  else
                     node ("Q") .url = new X3D .MFString ("javascript: function initialize () { print ('Q2 initialize'); } function set_z (value) { print ('Q2 set_z ' + value); } function shutdown () { print ('Q2 shutdown'); }");

                  note ("x_changed done");
               });

               note ("set_x 1");
               S .set_x = 1;
               note ("after the assignment");
               await spin (4);
               out .final = names (scene () .rootNodes);
               break;
            }
            case "disposeInCall":
            {
               window .removeCanvas = () => canvas .remove ();
               await spin (3);
               seq .length = 0;
               note ("set_x 1");
               node ("S") .set_x = 1;
               note ("after the assignment");
               await spin (12);
               break;
            }
            case "promiseJobs":
            {
               note ("set_x 1");
               node ("S") .set_x = 1;
               note ("after the assignment");
               await Promise .resolve ();
               note ("page microtask");
               await spin (3);
               note ("set_a 1");
               node ("S") .set_a = 1;
               note ("after set_a");
               await Promise .resolve ();
               note ("page microtask");
               await spin (3);
               break;
            }
            case "rootScript":
            {
               scene () .rootNodes .addFieldCallback ("page", v => note (`rootNodes callback ${v .length} ${names (v)}`));
               out .initial = names (scene () .rootNodes);
               note ("page addRootNode");
               out .add = attempt (() => scene () .addRootNode (make ("Group { }")));
               await spin (3);
               note ("page push");
               out .push = attempt (() => scene () .rootNodes .push (make ("Transform { }")));
               await spin (3);
               note ("page removeRootNode");
               out .remove = attempt (() => scene () .removeRootNode (node ("X")));
               await spin (3);
               note ("page rootNodes assign");
               out .assign = attempt (() => { scene () .rootNodes = new X3D .MFNode (node ("S"), node ("A")); });
               await spin (3);
               note ("Script read");
               node ("S") .read = 1;
               await spin (3);
               note ("Script add");
               node ("S") .add = 2;
               await spin (3);
               out .final = names (scene () .rootNodes);
               out .vrml  = scene () .toVRMLString () .replace (/\s+/g, " ") .trim () .replace (/ url "[^"]*"/, " url <code>");
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

// The Script of the root nodes cases is in a Group; `rootScript` (issue #202) keeps the Script itself as a root node. The `array[i].set is
// not a function` that was reported for that did not appear in X_ITE 16.5.0.
const WORLD_OF = name => name .startsWith ("chain") ? "chainWorld" : name .startsWith ("shutdown") ? "shutdown" : name .startsWith ("rootNodes") ? "rootNodes" : name .startsWith ("importJs") ? "importJson" : name .startsWith ("importXml") ? "importXml" : name;

const cases = Object .fromEntries ([ ... Object .keys (WORLDS) .filter (name => ![ "rootNodes", "importXml", "importJson", "chainWorld" ] .includes (name)), "chainRoots", "chainUrl", "shutdownUrl", "shutdownDispose", "importJsString", "importJsObject", "importXmlString", "importXmlDocument", "importXmlElement", ... [ "Page", "Add", "Remove", "Set", "Push" ] .map (how => `rootNodes${how}`) ] .map (name => [ name, page => scenario (page, name) ]));

// --- The reload sets (issue #202, item 4) ---------------------------------------------------------------------------------
//
// oracle/synthetic-inline-reload and oracle/synthetic-externproto-reload are step scenarios: each frame of a scenario has
// actions on an Inline (or an EXTERNPROTO), and the native suites compare them frame by frame with the pinned X_ITE's
// stepped page (oracle/x_ite-inline.mjs). A real page cannot step the frames, so these cases drive a scenario with the
// page's API only and compare what is settled: after the world and after each frame that has actions, they wait 400 ms, then
// in windows of 650 ms (longer than the 0.5 s cycle of the worlds' clocks) until two windows in a row have the same non-empty set of clock ticks (at most about 5 s, as a slow runner starts
// and stops clocks late); the ticks of the last window count. They record the console lines that name a state (`warn`, `error`, and
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
// The most windows a step waits for its clock ticks to settle (about 5 s): a slow machine starts and stops clocks late.
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
   // the step waits in windows of WINDOW_MS until two windows in a row have the same non-empty set of ticks (a slow machine starts
   // the clocks late and stops the old scene's clocks late; at most MAX_WINDOWS, for a step in which nothing ticks), and the ticks
   // are that set.
   const settle = () => page .evaluate (async ([ early, win, max ]) =>
   {
      const r    = window .__reload;
      const wait = ms => new Promise (resolve => setTimeout (resolve, ms));
      const set  = from => [ ... new Set (r .lines .slice (from) .filter (line => !r .states (line)) .map (line => line .replace (/\d+(\.\d+)?/g, "N"))) ] .sort () .join ("\n");

      await wait (early);

      let previous = null;

      for (let i = 0; i < max; ++ i)
      {
         r .mark = r .lines .length;
         await wait (win);

         const current = set (r .mark);

         if (current && current === previous)
            break;

         previous = current;
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

// --- A failed response and the next load attempt (issue #202, c17) ----------------------------------------------------------
//
// pinned X_ITE fetches again at each load of an Inline (`fetch` with `cache: "default"` over a `no-store` server), so an Inline
// whose file was published after a 404 loads it at the next attempt; a failed url that nothing loads again is not asked for again.
// An EXTERNPROTO caches its scene for good (FileLoader's static scene cache), a failure too. The cases wait for events (the
// console lines of the page), never for a time: what must not happen is shown by a later load (an Inline of another failed url) that
// has finished, twice.
//
// The worlds and the files are published by the server while the case runs (`publish`), the cases count the requests of one path.
const RETRY_LATE   = "/synthetic-retry/late.wrl";
const RETRY_LIB    = "/synthetic-retry/lib.wrl";
const RETRY_BODIES = {
   late: `#VRML V2.0 utf8\nDEF L Script { url "javascript:\nfunction initialize () { print ('L initialize'); }\n" }\n`,
   lib:  `#VRML V2.0 utf8\nPROTO G [ ] { DEF S Script { url "javascript:\nfunction initialize () { print ('G initialize'); }\n" } }\n`,
};

async function retryCase (page, env, name)
{
   const server = env .server;
   const origin = env .base;
   const world  = `${origin}/synthetic-retry/main.wrl`;
   const externproto = name .startsWith ("externproto");
   const scene  = `#VRML V2.0 utf8\nEXTERNPROTO G [ ] "${origin}${RETRY_LIB}#G"\nDEF I G { }\n`;
   const text   = `#VRML V2.0 utf8\nDEF Inl Inline { url "${externproto ? "a.wrl" : "late.wrl"}" }\nDEF Other Inline { url "other0.wrl" }\n`;

   server .unpublish ();
   server .publish ("/synthetic-retry/main.wrl", Buffer .from (text));

   // The EXTERNPROTO cases: two scenes (a Inline's files) that use the same EXTERNPROTO, whose file is not there yet.
   if (externproto)
      for (const file of [ "a", "b" ])
         server .publish (`/synthetic-retry/${file}.wrl`, Buffer .from (scene));

   // The server's log has the requests of the earlier runs and cases.
   const first = server .requests .length;

   await page .evaluate (async ([ url ]) =>
   {
      const X3D     = window .X3D_NS;
      const canvas  = document .querySelector ("x3d-canvas");
      const browser = X3D .getBrowser (canvas);
      const lines   = [ ];
      const origin  = window .location .origin;

      canvas .setAttribute ("scripts", "true");
      window .__retry = { lines, count: 0 };

      for (const type of [ "log", "warn", "error" ])
         console [type] = (... args) => lines .push (`${type}: ${args .join (" ") .split ("\n") [0] .split (origin) .join ("http://oracle")}`);

      window .addEventListener ("error", event => { lines .push (`pageerror: ${String (event .error ?.message ?? event .message)}`); event .preventDefault (); });

      try { await browser .loadURL (new X3D .MFString (url)); }
      catch (error) { lines .push (`loadURL rejected: ${String (error ?.message ?? error)}`); }
   }, [ world ]);

   // Polls for a line of the page (up to 20 s), and returns whether it came.
   const until = (pattern, from) => page .evaluate (async ([ pattern, from ]) =>
   {
      const lines = window .__retry .lines;
      const re    = new RegExp (pattern);

      for (let i = 0; i < 400; ++ i)
      {
         if (lines .slice (from) .some (line => re .test (line)))
            return true;

         await new Promise (resolve => setTimeout (resolve, 50));
      }

      return false;
   }, [ pattern, from ]);

   const mark   = () => page .evaluate (() => window .__retry .lines .length);
   const count  = path => server .requests .slice (first) .filter (r => r .path === path) .length;
   const result = { steps: [ ] };

   // Another Inline fails to load, twice in a row (each with a url that was not asked for yet): every request made before the first of
   // them is settled by then, so a request that must not be made is not made.
   const barrier = async () =>
   {
      for (let i = 0; i < 2; ++ i)
      {
         const from = await mark ();
         const url  = `other${++ window_counter}.wrl`;

         await page .evaluate (url => { window .X3D_NS .getBrowser (document .querySelector ("x3d-canvas")) .currentScene .getNamedNode ("Other") .url = new window .X3D_NS .MFString (url); }, url);
         await until (`other${window_counter}\\.wrl`, from);
      }
   };

   let window_counter = 0;

   const step = async (label, action, expected) =>
   {
      const from = result .steps .length ? await mark () : 0;

      if (action)
         await action ();

      const came = expected ? await until (expected, from) : true;

      await barrier ();

      const lines = await page .evaluate (from => window .__retry .lines .slice (from) .filter (line => /^(error|warn):|initialize/ .test (line)) .map (line => line .replace (/other\d+\.wrl/g, "otherN.wrl")), from);

      result .steps .push ({ label, came, requests: count (externproto ? RETRY_LIB : RETRY_LATE), lines: lines .filter (line => ! line .includes ("otherN.wrl")) });
   };

   const set = value => () => page .evaluate (([ name, value ]) =>
   {
      const X3D  = window .X3D_NS;
      const node = X3D .getBrowser (document .querySelector ("x3d-canvas")) .currentScene .getNamedNode (name);

      if (typeof value === "boolean")
         node .load = value;
      else
         node .url = new X3D .MFString (... value);
   }, [ "Inl", value ]);

   const publish = () => { server .publish (externproto ? RETRY_LIB : RETRY_LATE, Buffer .from (externproto ? RETRY_BODIES .lib : RETRY_BODIES .late)); };

   await until (externproto ? "EXTERNPROTO|Couldn't load URL .*lib" : "Couldn't load URL .*late", 0);
   await step ("failed", null, null);

   publish ();

   switch (name)
   {
      // Nothing sets the url again: the failed url is not asked for again.
      case "inlineNotSetAgain":
         await step ("published", null, null);
         break;
      // The url is set again (to the same url): the Inline loads the published file.
      case "inlineSameUrl":
         await step ("url again", set ([ "late.wrl" ]), "L initialize");
         break;
      // `load` is switched off and on again.
      case "inlineLoadToggle":
         await step ("load false", set (false), null);
         await step ("load true", set (true), null);
         break;
      // The Inline's url is set to another scene with the same EXTERNPROTO after the file was published: the Inline's
      // attempt starts over, the EXTERNPROTO keeps its failure.
      case "externprotoAgain":
         await step ("again", set ([ "b.wrl" ]), "G initialize|Couldn't load URL .*lib");
         break;
   }

   server .unpublish ();

   return result;
}

const retryCases = Object .fromEntries ([ "inlineNotSetAgain", "inlineSameUrl", "inlineLoadToggle", "externprotoAgain" ] .map (name => [ name, (page, env) => retryCase (page, env, name) ]));

export const CASE_GROUPS = {
   inlineRetry: { context: DESKTOP, cases: retryCases },
   scriptLife: { context: DESKTOP, cases },
   scriptReload: { context: DESKTOP, cases: reloadCases },
};
