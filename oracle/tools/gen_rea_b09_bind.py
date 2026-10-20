#!/usr/bin/env python3
"""Generate the bindable-stack worlds oracle/synthetic-rea-b09-sai/bind-*.wrl and assets/bind-*.wrl (REA batch 09, step 3, item 3).
Authored; synthetic inputs only.

Each world has bindable nodes A, B, C of one type, a Script P whose `b_X` / `t_X` inputs are routed from X.isBound / X.bindTime and append
`X:1@step` / `X:0@step` / `X:T@step` to its `hist` field (so the order of the events and the frame they arrive in is in the scene text), and
a list of actions run at fixed steps: set_bind true / false, removing a bound node from the root nodes, creating and adding a node,
an Inline whose bindable node is imported, ...
"""
import os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")
asset_dir = os.path.join(out_dir, "assets")


def vs(js):
    return js.replace("\\", "\\\\").replace('"', '\\"')


# type -> (component line, node source with a distinguishing field)
TYPES = {
    "Viewpoint": ("", 'Viewpoint { description "%s" position 0 0 %d }'),
    "OrthoViewpoint": ("", 'OrthoViewpoint { description "%s" position 0 0 %d }'),
    "GeoViewpoint": ("COMPONENT Geospatial:1\n", 'GeoViewpoint { description "%s" position 0 0 %d }'),
    "NavigationInfo": ("", 'NavigationInfo { type "%s" speed %d }'),
    "Background": ("", 'Background { skyColor 0 0 %d }'),
    "TextureBackground": ("COMPONENT EnvironmentalEffects:3\n", 'TextureBackground { skyColor 0 0 %d }'),
    "Fog": ("", 'Fog { color 0 0 %d }'),
}


def node_src(t, name, i):
    tmpl = TYPES[t][1]

    if t == "NavigationInfo":
        return f"DEF {name} " + tmpl % (["WALK", "FLY", "EXAMINE"][i], i + 1)

    if t in ("Background", "TextureBackground", "Fog"):
        return f"DEF {name} " + tmpl % ((i + 1) / 4)

    return f"DEF {name} " + tmpl % (name, 10 + i)


def script(names, actions, header="", extra_fields="", extra_js=""):
    fields = 'inputOutput SFString hist ""\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n'
    inputs = ""

    for n in names:
        fields += f"  inputOnly SFBool b_{n}\n  inputOnly SFTime t_{n}\n"
        inputs += f"function b_{n} (v) {{ rec ('{n}:' + (v ? 1 : 0)); }}\nfunction t_{n} (v) {{ rec ('{n}:T' + (v > 0 ? '' : '?')); }}\n"

    body = "".join(f"  if (step == {s}) p ('{label}', function () {{ {code} }});\n" for s, label, code in actions)
    js = f"""
function rec (s) {{ hist = hist + s + '@' + step + ' '; }}
function p (name, f) {{ var v; try {{ v = f (); }} catch (e) {{ v = 'ERR:' + e .message; }} hist = hist + '[' + name + (v === undefined ? '' : '=' + v) + ']@' + step + ' '; }}
{inputs}{extra_js}
function cycle () {{
  step = step + 1;
  var s = Browser .currentScene;
{body}}}"""
    return f'DEF P Script {{\n  {fields}{extra_fields}  url "ecmascript:\n{vs(js)}\n  "\n}}\n'


def routes(names):
    return "".join(f"ROUTE {n}.isBound TO P.b_{n}\nROUTE {n}.bindTime TO P.t_{n}\n" for n in names)


def world(title, t, nodes, names, actions, post="", pre="", extra_fields="", extra_js="", comp=None):
    head = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n" + (TYPES[t][0] if comp is None else comp)
    return (head + f'META "title" "{title}"\n' + pre + nodes + script(names, actions, extra_fields=extra_fields, extra_js=extra_js)
            + "DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }\nROUTE STEP.cycleTime TO P.cycle\n" + routes(names) + post)


def sb(n, v, f="set_bind"):
    return f"s .getNamedNode ('{n}') .{f} = {'true' if v else 'false'};"


cases = {}
assets = {}
N = ["A", "B", "C"]

for t in TYPES:
    nodes = "".join(node_src(t, n, i) + "\n" for i, n in enumerate(N))
    actions = [
        (2, "bindB", sb("B", True)),
        (3, "bindC", sb("C", True)),
        (4, "bindA", sb("A", True)),
        (5, "unbindC", sb("C", False)),
        (6, "unbindA", sb("A", False)),
        (7, "rebindB", sb("B", True)),
        (8, "unbindC2", sb("C", False)),
        (9, "unbindB", sb("B", False)),
        (10, "bindAB", sb("A", True) + " " + sb("B", True)),
        (11, "unbindAB", sb("A", False) + " " + sb("B", False)),
        (12, "bindBA", sb("B", True) + " " + sb("A", True) + " " + sb("A", False) + " " + sb("A", True)),
        (13, "removeA", "s .removeRootNode (s .getNamedNode ('A')); return s .rootNodes .length;"),
        (14, "removeB", "s .removeRootNode (s .getNamedNode ('B')); return s .rootNodes .length;"),
        (15, "bindC", sb("C", True)),
        (16, "removeC", "s .removeRootNode (s .getNamedNode ('C')); return s .rootNodes .length;"),
        (17, "create", f"var n = s .createNode ('{t}'); s .addRootNode (n); n .set_bind = true; return n .isBound;"),
        (18, "state", "return s .rootNodes .length;"),
    ]
    cases[f"bind-{t}"] = world(f"bind {t}", t, nodes, N, actions)

# Bound from a ROUTE instead of a Script write; and bind events sent while the node is part of an Inline scene.
t = "Viewpoint"
cases["bind-route"] = world("bind route", t, "".join(node_src(t, n, i) + "\n" for i, n in enumerate(N)) + "DEF BV BooleanToggle { }\n", N,
                            [(2, "toggle", "s .getNamedNode ('BV') .set_boolean = true;"),
                             (3, "toggle", "s .getNamedNode ('BV') .set_boolean = true;"),
                             (4, "toggle", "s .getNamedNode ('BV') .set_boolean = true;")],
                            post="DEF TRUEB BooleanFilter { }\nROUTE BV.toggle TO B.set_bind\n", comp="")
# A BooleanToggle inverts its stored value on set_boolean; BooleanFilter is unused here.

assets["bind-inline.wrl"] = ("#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"
                             'DEF IV Viewpoint { description "iv" position 0 0 20 }\nDEF IV2 Viewpoint { description "iv2" position 0 0 21 }\n'
                             'DEF IB Background { skyColor 1 1 0 }\nDEF IN NavigationInfo { type "FLY" }\nEXPORT IV\nEXPORT IV2\nEXPORT IB\nEXPORT IN\n')
imp = "DEF IL Inline { url \"assets/bind-inline.wrl\" }\nIMPORT IL.IV AS IV\nIMPORT IL.IV2 AS IV2\nIMPORT IL.IB AS IB\nIMPORT IL.IN AS IN\n"
nodes = node_src("Viewpoint", "A", 0) + "\n" + node_src("Viewpoint", "B", 1) + "\n" + node_src("Background", "BG", 0) + "\n" + node_src("NavigationInfo", "NI", 1) + "\n"
names = ["A", "B", "IV", "IV2", "BG", "IB", "NI", "IN"]
cases["bind-inline"] = world("bind inline", "Viewpoint", nodes + imp, names, [
    (6, "state", "return s .getImportedNode ('IV') .getNodeTypeName();"),
    (7, "bindIV", "s .getImportedNode ('IV') .set_bind = true;"),
    (8, "bindIV2", "s .getImportedNode ('IV2') .set_bind = true;"),
    (9, "bindA", sb("A", True)),
    (10, "bindIB", "s .getImportedNode ('IB') .set_bind = true;"),
    (11, "bindIN", "s .getImportedNode ('IN') .set_bind = true;"),
    (12, "unbindIV2", "s .getImportedNode ('IV2') .set_bind = false;"),
    (13, "unload", "s .getNamedNode ('IL') .load = false;"),
    (15, "bindB", sb("B", True)),
    (16, "reload", "s .getNamedNode ('IL') .load = true;"),
    (22, "bindIV", "s .getImportedNode ('IV') .set_bind = true;"),
    (23, "unbindA", sb("A", False)),
    (24, "unbindB", sb("B", False)),
], comp="")

# Two stacks and the node's own view of it: isBound read back, bindTime ordering relative to isBound.
cases["bind-read"] = world("bind read", "Viewpoint", "".join(node_src("Viewpoint", n, i) + "\n" for i, n in enumerate(N)), N, [
    (2, "read0", "var r = []; N .forEach (function (n) { r .push (n + (s .getNamedNode (n) .isBound ? 1 : 0)); }); return r .join (',');"),
    (3, "bindB", sb("B", True) + " var r = []; N .forEach (function (n) { r .push (n + (s .getNamedNode (n) .isBound ? 1 : 0)); }); return r .join (',');"),
    (4, "read1", "var r = []; N .forEach (function (n) { r .push (n + (s .getNamedNode (n) .isBound ? 1 : 0)); }); return r .join (',');"),
    (5, "bindTimeSet", "return typeof s .getNamedNode ('B') .bindTime + (s .getNamedNode ('B') .bindTime > 0) + (s .getNamedNode ('A') .bindTime > 0) + (s .getNamedNode ('C') .bindTime > 0);"),
    (6, "firstTwice", "s .getNamedNode ('C') .set_bind = true; s .getNamedNode ('C') .set_bind = true;"),
    (7, "setBindNonBool", "s .getNamedNode ('A') .set_bind = 1; return s .getNamedNode ('A') .isBound;"),
    (8, "viewpoints", "return Browser .currentScene .getNamedNode ('A') .getNodeTypeName();"),
], extra_js="var N = ['A', 'B', 'C'];\n", comp="")

# Viewpoint with jump FALSE / retainUserOffsets, and bind via Browser calls.
cases["bind-browser"] = world("bind browser", "Viewpoint", "".join(node_src("Viewpoint", n, i) + "\n" for i, n in enumerate(N)), N, [
    (2, "next", "Browser .nextViewpoint ();"),
    (3, "next", "Browser .nextViewpoint ();"),
    (4, "next", "Browser .nextViewpoint ();"),
    (5, "prev", "Browser .previousViewpoint ();"),
    (6, "first", "Browser .firstViewpoint ();"),
    (7, "last", "Browser .lastViewpoint ();"),
    (8, "change", "Browser .changeViewpoint ('B');"),
    (9, "changeMissing", "Browser .changeViewpoint ('NOPE');"),
    (10, "changeDesc", "Browser .changeViewpoint ('A');"),
], comp="")

# Layers: every layer has its own stacks; the world is layer 0.
LAYERS = ("#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\nCOMPONENT Layering:1\n"
          'META "title" "bind layers"\n'
          "DEF LS LayerSet { activeLayer 1 order [ 0 1 2 ] layers [\n"
          "  DEF L1 Layer { children [ DEF A Viewpoint { description \"a\" } DEF B Viewpoint { description \"b\" position 0 0 12 } DEF NA NavigationInfo { type \"FLY\" } ] }\n"
          "  DEF L2 Layer { children [ DEF C Viewpoint { description \"c\" position 0 0 13 } DEF D Viewpoint { description \"d\" position 0 0 14 } DEF NB NavigationInfo { type \"EXAMINE\" } ] }\n"
          "] }\n"
          "DEF W Viewpoint { description \"w\" position 0 0 5 }\n")
LN = ["A", "B", "C", "D", "W", "NA", "NB"]
LAYERS += script(LN, [
    (2, "bindB", sb("B", True)),
    (3, "bindD", sb("D", True)),
    (4, "bindW", sb("W", True)),
    (5, "active2", "s .getNamedNode ('LS') .activeLayer = 2;"),
    (6, "bindA", sb("A", True)),
    (7, "bindC", sb("C", True)),
    (8, "next", "Browser .nextViewpoint ();"),
    (9, "active0", "s .getNamedNode ('LS') .activeLayer = 0;"),
    (10, "next", "Browser .nextViewpoint ();"),
    (11, "unbindW", sb("W", False)),
    (12, "bindNA", sb("NA", True)),
    (13, "bindNB", sb("NB", True)),
    (14, "unbindNA", sb("NA", False)),
    (15, "removeLayer", "var ls = s .getNamedNode ('LS'); ls .layers = new MFNode (s .getNamedNode ('L1'));"),
    (16, "addBack", "var ls = s .getNamedNode ('LS'); ls .layers = new MFNode (s .getNamedNode ('L1'), s .getNamedNode ('L2'));"),
    (17, "bindD", sb("D", True)),
]) + "DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }\nROUTE STEP.cycleTime TO P.cycle\n" + routes(LN)
cases["bind-layers"] = LAYERS

# World switch: A binds, then loads B; B (a world of its own, with its own Script) records its bind events in hist, A logs to the console.
HB = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"
BN = ["BA", "BB", "BG1", "BN1", "BF1"]
assets["bind-b.wrl"] = (HB + 'DEF BA Viewpoint { description "ba" position 0 0 7 }\nDEF BB Viewpoint { description "bb" position 0 0 8 }\n'
                        "DEF BG1 Background { skyColor 0 1 0 }\nDEF BN1 NavigationInfo { type \"EXAMINE\" }\nDEF BF1 Fog { color 0 1 0 }\n"
                        + script(BN, [(2, "bindBB", "Browser .currentScene .getNamedNode ('BB') .set_bind = true;")]).replace("function cycle", "function initialize () { rec ('init'); }\nfunction cycle")
                        + "DEF STEPB TimeSensor { cycleInterval 0.05 loop TRUE }\nROUTE STEPB.cycleTime TO P.cycle\n" + routes(BN))
AN = ["A", "B", "AG", "AN", "AF"]
a_nodes = (node_src("Viewpoint", "A", 0) + "\n" + node_src("Viewpoint", "B", 1) + "\nDEF AG Background { skyColor 1 0 0 }\nDEF AN NavigationInfo { type \"FLY\" }\nDEF AF Fog { color 1 0 0 }\n")
cases["bind-switch"] = world("bind switch", "Viewpoint", a_nodes, AN, [
    (2, "bindB", sb("B", True)),
    (3, "load", "Browser .loadURL (new MFString ('assets/bind-b.wrl')); console .log ('A: loadURL called'); return 'x';"),
], comp="").replace("function rec (s) { hist = hist + s + '@' + step + ' '; }", "function rec (s) { hist = hist + s + '@' + step + ' '; console .log ('A: ' + s + '@' + step); }")

if __name__ == "__main__":
    os.makedirs(asset_dir, exist_ok=True)

    for name, text in cases.items():
        with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
            f.write(text)

    for name, text in assets.items():
        with open(os.path.join(asset_dir, name), "w") as f:
            f.write(text)

    print(len(cases), "worlds")
