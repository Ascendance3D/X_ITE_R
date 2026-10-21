#!/usr/bin/env python3
"""Generate the field callback probe worlds oracle/synthetic-rea-b09-sai/fcb-*.wrl (REA batch 09 cloud, sub:browser.sai-api,
sub:fields.types). Authored; synthetic inputs only.

X3DField.addFieldCallback / removeFieldCallback / getFieldCallbacks on ordinary node fields and on Script fields, and
SFNode.addFieldCallback (key, name, fn) / removeFieldCallback (key, name). X3DField.processEvent runs the callbacks with
`this .valueOf ()` after it has passed the event on to the routes. A driver Script `D` acts in prepareEvents of frame k
(its counter F); every observation is appended to the log and copied into the MFString `out` of `D`, so
the scene text of every frame shows what ran and when.

  python3 -I oracle/tools/gen_rea_b09_fieldcb.py
"""
import os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")
HEAD = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"

COMMON = r"""
var F = 0;
function L (s) { var a = globalThis .LOG_ = globalThis .LOG_ || []; a .push (F + ':' + s); out = new MFString (... a); }
function show (v) {
  if (v === null || v === undefined) return String (v);
  if (typeof v === 'number') return Object .is (v, -0) ? 'n:-0' : 'n:' + v;
  if (typeof v === 'string') return 's:' + JSON .stringify (v);
  if (typeof v === 'boolean') return 'b:' + v;
  if (typeof v === 'function') return 'fn';
  var c = v .constructor;
  return (c && c .name || '?') + ':' + String (v);
}
function E (e) { return 'ERR ' + (e && e .name) + ': ' + (e && e .message); }
function S (n) { return Browser .currentScene .getNamedNode (n); }
"""


def vs(js):
    return js.replace("\\", "\\\\").replace('"', '\\"')


def world(title, nodes, steps, extra_js="", fields="", routes=""):
    """`steps`: {frame: js} run by D's prepareEvents in that frame (frame 1 is the first prepareEvents after initialize)."""
    body = "\n".join(f"  if (F === {k}) {{ try {{ {js} }} catch (e) {{ L ('step {k} ' + E (e)); }} }}" for k, js in sorted(steps.items()))
    js = f"{COMMON}\n{extra_js}\nfunction prepareEvents () {{\n  ++ F;\n{body}\n}}\n"
    return (HEAD + f'META "title" "{title}"\n' + nodes +
            f'DEF D Script {{\n  directOutput TRUE\n  mustEvaluate TRUE\n  inputOutput MFString out [ ]\n{fields}  url "ecmascript:\n{vs(js)}\n  "\n}}\n' + routes)


W = {}

# 1. The callback API on a node's fields: who is called, with what, in which frame; replace, remove, several keys.
W["fcb-basic"] = world("fcb basic", """DEF T Transform { }
DEF B TouchSensor { }
DEF M Material { }
DEF ST WorldInfo { }
DEF C Coordinate { point [ 0 0 0 ] }
""", {
    1: "var t = S ('T'); t .addFieldCallback ('k1', 'translation', function (v) { L ('T.translation k1 ' + show (v) + ' same ' + (v === t .translation)); });"
       " L ('added'); t .translation = new SFVec3f (1, 2, 3); L ('assigned');",
    2: "var t = S ('T'); t .addFieldCallback ('k2', 'translation', function (v) { L ('T.translation k2 ' + show (v)); }); t .translation .x = 5; L ('x set');",
    3: "var t = S ('T'); t .addFieldCallback ('k1', 'translation', function (v) { L ('T.translation k1 replaced ' + show (v)); }); t .translation = new SFVec3f (7, 8, 9);",
    4: "var t = S ('T'); t .removeFieldCallback ('k2', 'translation'); t .translation = new SFVec3f (0, 0, 1);",
    5: "var t = S ('T'); t .removeFieldCallback ('k1', 'translation'); t .removeFieldCallback ('nope', 'translation'); t .translation = new SFVec3f (0, 0, 2); L ('removed all');",
    6: "var m = S ('M'); m .addFieldCallback ('f', 'transparency', function (v) { L ('M.transparency ' + show (v)); });"
       " m .addFieldCallback ('c', 'diffuseColor', function (v) { L ('M.diffuseColor ' + show (v)); });"
       " m .transparency = 0.25; m .diffuseColor = new SFColor (1, 0, 0); m .transparency = 0.5;",
    7: "var w = S ('ST'); w .addFieldCallback ('s', 'title', function (v) { L ('W.title ' + show (v)); }); w .addFieldCallback ('i', 'info', function (v) { L ('W.info ' + show (v) + ' len ' + v .length); });"
       " w .title = 'hello'; w .info = new MFString ('a', 'b');",
    8: "var c = S ('C'); c .addFieldCallback ('p', 'point', function (v) { L ('C.point ' + v .length + ' ' + show (v [0])); }); c .point [0] = new SFVec3f (1, 1, 1); c .point .push (new SFVec3f (2, 2, 2));",
    9: "var b = S ('B'); b .addFieldCallback ('e', 'enabled', function (v) { L ('B.enabled ' + show (v)); }); b .enabled = false; b .enabled = true;",
    10: "var t = S ('T'); var f = t .translation; f .addFieldCallback ('fo', function (v) { L ('field object cb ' + show (v) + ' this ' + (this === undefined ? 'undef' : typeof this)); }); t .translation = new SFVec3f (3, 3, 3);",
    11: "var t = S ('T'); var f = t .translation; var m = f .getFieldCallbacks (); L ('getFieldCallbacks ' + (m instanceof Map) + ' ' + m .size + ' ' + Array .from (m .keys ()) .join (','));"
        " f .removeFieldCallback ('fo'); L ('after remove ' + f .getFieldCallbacks () .size); t .translation = new SFVec3f (4, 4, 4);",
    12: "var t = S ('T'); t .addFieldCallback ('again', 'translation', function (v) { L ('T again ' + show (v)); }); t .translation = t .translation; L ('self assign');",
    13: "try { S ('T') .addFieldCallback ('x', 'nosuch', function () { }); L ('unknown field ok'); } catch (e) { L ('unknown field ' + E (e)); }",
    14: "var r = S ('T') .addFieldCallback ('again', 'translation', function () { }); L ('return ' + show (r)); var q = S ('T') .removeFieldCallback ('again', 'translation'); L ('remove return ' + show (q));",
    15: "var n = new SFNode ('Transform { }'); var r = n .addFieldCallback ('k', function () { L ('two-arg sfnode cb'); }); L ('two-arg ' + show (r)); n .translation = new SFVec3f (1, 0, 0);",
})

# 2. Routes and the cascade: a routed event reaches the callback in the same processEvents run; order against the destination's
# own event handler and against a second hop.
W["fcb-routes"] = world("fcb routes", """DEF T1 Transform { }
DEF T2 Transform { }
DEF T3 Transform { }
DEF R Script {
  inputOnly SFVec3f set_v
  outputOnly SFVec3f v_changed
  url "ecmascript:
function set_v (v) { v_changed = v; }
  "
}
""", {
    1: "S ('T2') .addFieldCallback ('a', 'translation', function (v) { L ('T2.translation ' + show (v)); });"
       " S ('T3') .addFieldCallback ('a', 'translation', function (v) { L ('T3.translation ' + show (v)); });"
       " S ('T1') .addFieldCallback ('a', 'translation', function (v) { L ('T1.translation ' + show (v)); });"
       " S ('T1') .addFieldCallback ('b', 'translation_changed', function (v) { L ('T1.translation_changed ' + show (v)); });"
       " S ('R') .addFieldCallback ('a', 'v_changed', function (v) { L ('R.v_changed ' + show (v)); });"
       " S ('R') .addFieldCallback ('b', 'set_v', function (v) { L ('R.set_v ' + show (v)); });"
       " S ('T1') .translation = new SFVec3f (1, 0, 0); L ('set T1');",
    2: "S ('T1') .translation = new SFVec3f (2, 0, 0); S ('T1') .translation = new SFVec3f (3, 0, 0); L ('set T1 twice');",
    3: "Browser .currentScene .deleteRoute (Browser .currentScene .routes [0]); S ('T1') .translation = new SFVec3f (4, 0, 0); L ('route 0 deleted');",
}, routes="ROUTE T1.translation_changed TO T2.set_translation\nROUTE T1.translation_changed TO R.set_v\nROUTE R.v_changed TO T3.set_translation\n")

# 3. Callbacks that act: add another callback while running, remove themselves, set fields (their own and other ones), throw.
W["fcb-reentry"] = world("fcb reentry", """DEF T Transform { }
DEF U Transform { }
""", {
    1: "var t = S ('T'), u = S ('U');"
       " t .addFieldCallback ('adder', 'translation', function (v) { L ('adder ' + show (v)); t .addFieldCallback ('late', 'translation', function (w) { L ('late ' + show (w)); }); });"
       " t .addFieldCallback ('self', 'translation', function (v) { L ('self-remove ' + show (v)); t .removeFieldCallback ('self', 'translation'); });"
       " t .addFieldCallback ('setter', 'translation', function (v) { L ('setter ' + show (v)); if (v .x < 2) u .translation = new SFVec3f (v .x + 10, 0, 0); });"
       " u .addFieldCallback ('u', 'translation', function (v) { L ('U ' + show (v)); });"
       " t .translation = new SFVec3f (1, 0, 0);",
    2: "S ('T') .translation = new SFVec3f (2, 0, 0);",
    3: "var t = S ('T'); t .addFieldCallback ('loop', 'scale', function (v) { L ('loop ' + show (v)); if (v .x < 4) t .scale = new SFVec3f (v .x + 1, 1, 1); }); t .scale = new SFVec3f (2, 1, 1);",
})

# 4. The Script's own fields: callbacks on user-defined fields of D and of another Script, through the node and through the
# global field of the Script.
W["fcb-script-fields"] = world("fcb script fields", """DEF O Script {
  inputOutput SFInt32 io 0
  outputOnly SFString os
  inputOnly SFFloat inF
  url "ecmascript:
function set_io (v) { }
function inF (v) { os = 'got ' + v; }
  "
}
""", {
    1: "var o = S ('O');"
       " o .addFieldCallback ('a', 'io', function (v) { L ('O.io ' + show (v)); });"
       " o .addFieldCallback ('b', 'os', function (v) { L ('O.os ' + show (v)); });"
       " o .addFieldCallback ('c', 'inF', function (v) { L ('O.inF ' + show (v)); });"
       " o .io = 3; o .inF = 1.5;",
    2: "S ('D') .addFieldCallback ('mine', 'loc', function (v) { L ('D.loc ' + show (v)); }); loc = 7; L ('loc assigned');",
    3: "loc = 8; loc = 9;",
}, fields="  inputOutput SFInt32 loc 0\n")


# 5. A callback that throws: X3DField.processEvent does not catch it. What still runs after it: the next callback of the same field,
# the other events of the same frame, the next frame. (The exception itself is an uncaught page error, which the oracle page
# does not record; only its effects are compared.)
W["fcb-throw"] = world("fcb throw", """DEF T Transform { }
DEF U Transform { }
""", {
    1: "var t = S ('T'), u = S ('U');"
       " t .addFieldCallback ('a', 'translation', function (v) { L ('a ' + show (v)); });"
       " t .addFieldCallback ('b', 'translation', function (v) { L ('b throws'); throw new Error ('boom'); });"
       " t .addFieldCallback ('c', 'translation', function (v) { L ('c ' + show (v)); });"
       " u .addFieldCallback ('u', 'translation', function (v) { L ('U ' + show (v)); });"
       " t .translation = new SFVec3f (1, 0, 0); u .translation = new SFVec3f (2, 0, 0); L ('set both');",
    2: "S ('U') .translation = new SFVec3f (3, 0, 0); L ('set U');",
    3: "S ('T') .translation = new SFVec3f (4, 0, 0); L ('set T');",
})


if __name__ == "__main__":
    for name, text in W.items():
        with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
            f.write(text)
        print(name)
