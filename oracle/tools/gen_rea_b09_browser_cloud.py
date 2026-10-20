#!/usr/bin/env python3
"""Generate the Browser object survey worlds oracle/synthetic-rea-b09-sai/brx-*.wrl (REA batch 09 cloud, scr-browser-open).
Authored; synthetic inputs only. Uses the probe harness of gen_rea_b09_script.py (`T (label, fn)` into the MFString `out`).

In X_ITE a Script's `Browser` is the X3DBrowser itself (X3DBrowser → X3DBrowserContext → X3DBaseNode → X3DChildObject →
X3DObject). These worlds record its prototype chain and what the members scr-browser-open found missing return when they are
called. Members with side effects on the page (dispose, resize, addEvent) are only described, never called.

  python3 -I oracle/tools/gen_rea_b09_browser_cloud.py
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_rea_b09_script as G  # noqa: E402
from gen_rea_b09_fields_cloud import CHAIN  # noqa: E402

# What an object is: its constructor's name, its prototype chain's constructor names, its own keys.
DESC = ("function desc (o) { if (o === null || typeof o !== 'object' && typeof o !== 'function') return show (o); var n = [], p = o, k = 0; "
        "while (p && k ++ < 10) { n .push (p .constructor && Object .prototype .hasOwnProperty .call (p, 'constructor') ? p .constructor .name : '-'); p = Object .getPrototypeOf (p); } "
        "return [Object .prototype .toString .call (o), n .join ('>'), Object .keys (o) .sort () .join (',')]; }")

P_API = [
    ("Browser chain", "return chain (Browser);"),
    ("Browser desc", "return desc (Browser);"),
    ("Browser instanceof X3DBrowser", "return typeof X3DBrowser === 'function' && Browser instanceof X3DBrowser;"),
    ("Browser constructor === X3DBrowser", "return typeof X3DBrowser === 'function' && Browser .constructor === X3DBrowser;"),
    ("Browser toStringTag", "return Browser [Symbol .toStringTag];"),
    ("Browser JSON", "try { return JSON .stringify (Browser) .length > 0; } catch (e) { return e .name; }"),
    ("Browser keys", "return Object .keys (Browser) .sort ();"),
]

CALLS = [
    ("getCurrentViewer", "Browser .getCurrentViewer ()"),
    ("getViewer", "Browser .getViewer ()"),
    ("getActiveViewpoint", "Browser .getActiveViewpoint ()"),
    ("getActiveNavigationInfo", "Browser .getActiveNavigationInfo ()"),
    ("getActiveLayer", "Browser .getActiveLayer ()"),
    ("getBrowserOptions", "Browser .getBrowserOptions ()"),
    ("getBrowserProperties", "Browser .getBrowserProperties ()"),
    ("getRenderingProperties", "Browser .getRenderingProperties ()"),
    ("isContextLost", "Browser .isContextLost ()"),
    ("getCanvas", "Browser .getCanvas ()"),
    ("getElement", "Browser .getElement ()"),
    ("getLocalStorage", "Browser .getLocalStorage ()"),
    ("getNotification", "Browser .getNotification ()"),
    ("getContextMenu", "Browser .getContextMenu ()"),
    ("getBrowserTime", "Browser .getBrowserTime ()"),
    ("getBrowserTimings", "Browser .getBrowserTimings ()"),
    ("toVRMLString", "Browser .toVRMLString ()"),
    ("toXMLString", "Browser .toXMLString ()"),
    ("toJSONString", "Browser .toJSONString ()"),
    ("getTypeName", "Browser .getTypeName ()"),
    ("getType", "Browser .getType ()"),
    ("getId", "typeof Browser .getId ()"),
    ("getFieldDefinitions", "Array .from (Browser .getFieldDefinitions (), function (d) { return d .name + ':' + d .dataType; })"),
    ("getFields", "Array .from (Browser .getFields (), function (f) { return f .getName () + ':' + f .getTypeName (); })"),
    ("getUserDefinedFields", "Array .from (Browser .getUserDefinedFields (), function (f) { return f .getName (); })"),
    ("getField missing", "Browser .getField ('nothing')"),
]

P_CALL = []
for name, expr in CALLS:
    P_CALL.append((f"{name} length", f"return Browser .{name.split()[0]} .length;"))
    P_CALL.append((f"{name} ()", f"return show ({expr});"))
    P_CALL.append((f"{name} () desc", f"return desc ({expr});"))
for name in ["dispose", "resize", "addEvent", "bindViewpoint"]:
    P_CALL.append((f"{name} length", f"return Browser .{name} .length;"))
    P_CALL.append((f"{name} owner", f"var p = Browser; while (p && !Object .prototype .hasOwnProperty .call (p, '{name}')) p = Object .getPrototypeOf (p); return p && p .constructor .name;"))

# The objects a Browser call returns: what a Script can do with them.
P_OBJ = [
    ("options type", "var o = Browser .getBrowserOptions (); return [o .getTypeName (), o .getType ? Array .from (o .getType ()) .length : -1];"),
    ("options fields", "var o = Browser .getBrowserOptions (); return Array .from (o .getFieldDefinitions (), function (d) { return d .name; });"),
    ("options field value", "var o = Browser .getBrowserOptions (); return [show (o .getField ('Shading')), show (o .getField ('Antialiased')), String (o .getField ('Shading') .getValue ())];"),
    ("options === getBrowserOptions", "return Browser .getBrowserOptions () === Browser .getBrowserOptions ();"),
    ("properties fields", "var o = Browser .getBrowserProperties (); return Array .from (o .getFieldDefinitions (), function (d) { return d .name + '=' + o .getField (d .name); });"),
    ("rendering fields", "var o = Browser .getRenderingProperties (); return Array .from (o .getFieldDefinitions (), function (d) { return d .name; });"),
    ("viewpoint getTypeName", "var v = Browser .getActiveViewpoint (); return [v .getTypeName (), v .getName ()];"),
    ("viewpoint equals SFNode", "var v = Browser .getActiveViewpoint (); return [v === Browser .currentScene .getNamedNode ('V1'), v === Browser .currentScene .getNamedNode ('V1') .getValue ()];"),
    ("viewpoint field", "var v = Browser .getActiveViewpoint (); return show (v .getField ('description'));"),
    ("navigationInfo getTypeName", "var v = Browser .getActiveNavigationInfo (); return [v .getTypeName (), v .getName ()];"),
    ("layer getTypeName", "var v = Browser .getActiveLayer (); return v && [v .getTypeName (), v .getName ()];"),
    ("getScriptNode", "var n = Browser .getScriptNode (); return [n .getTypeName (), n .getName (), n === SELF, n === SELF .getValue ()];"),
    ("getExecutionContext", "var c = Browser .getExecutionContext (); return [desc (c), c === Browser .currentScene];"),
    ("currentScene desc", "return desc (Browser .currentScene);"),
    ("browserTime vs time", "return [typeof Browser .getBrowserTime (), Browser .getBrowserTime () > 0];"),
]

# Field definitions: name, access type, data type, value.
DEFS = ("function defs (o) { return Array .from (o .getFieldDefinitions (), function (d) { return [d .name, d .accessType, d .dataType, "
        "String (d .value)] .join (' '); }); }")

P_DATA = [
    ("Browser definitions", "return defs (Browser);"),
    ("Browser getType", "return Array .from (Browser .getType ());"),
    ("X3DConstants.X3DBrowser", "return X3DConstants .X3DBrowser;"),
    ("options definitions", "return defs (Browser .getBrowserOptions ());"),
    ("options getType", "return Array .from (Browser .getBrowserOptions () .getType ());"),
    ("properties definitions", "return defs (Browser .getBrowserProperties ());"),
    ("rendering definitions", "return defs (Browser .getRenderingProperties ());"),
    # The GPU's limits (MaxTextureSize, MaxSamples …) are the machine's, not X_ITE's: only their types are compared.
    ("rendering values", "var o = Browser .getRenderingProperties (), gpu = ['MaxTextureSize', 'TextureUnits', 'MaxLights', 'ColorDepth', 'TextureMemory', 'MaxAnisotropicDegree', 'MaxSamples']; return Array .from (o .getFieldDefinitions (), function (d) { var f = o .getField (d .name); return d .name + '=' + (gpu .indexOf (d .name) < 0 ? f : typeof f .getValue ()); });"),
    ("options values", "var o = Browser .getBrowserOptions (); return Array .from (o .getFieldDefinitions (), function (d) { return d .name + '=' + o .getField (d .name); });"),
    ("activeViewpoint field", "var f = Browser .getField ('activeViewpoint'); return [f .getTypeName (), f .getAccessType (), String (f .getValue ())];"),
    ("getCurrentViewer", "return Browser .getCurrentViewer ();"),
    ("getExecutionContext === currentScene", "return Browser .getExecutionContext () === Browser .currentScene;"),
    ("getExecutionContext rootNodes", "return Browser .getExecutionContext () .rootNodes .length;"),
]

# The same at frame 5, once the world is installed and its bindables are bound.
P_LATER = [
    ("later getExecutionContext === currentScene", "return Browser .getExecutionContext () === Browser .currentScene;"),
    ("later getActiveViewpoint", "var v = Browser .getActiveViewpoint (); return [v .getTypeName (), v .getName (), v === Browser .currentScene .getNamedNode ('V1') .getValue ()];"),
    ("later getActiveNavigationInfo", "var v = Browser .getActiveNavigationInfo (); return [v .getTypeName (), v .getName (), v === Browser .currentScene .getNamedNode ('N1') .getValue ()];"),
    ("later getActiveLayer", "var v = Browser .getActiveLayer (); return v && [v .getTypeName (), v .getName ()];"),
    ("later getCurrentViewer", "return Browser .getCurrentViewer ();"),
    ("later activeViewpoint field", "return String (Browser .getField ('activeViewpoint') .getValue ());"),
    ("later String (V1 node)", "return String (Browser .currentScene .getNamedNode ('V1') .getValue ());"),
    ("later String (getActiveViewpoint)", "return String (Browser .getActiveViewpoint ());"),
    ("later field value === active", "return Browser .getField ('activeViewpoint') .getValue () === Browser .getActiveViewpoint ();"),
    ("later Browser.activeViewpoint", "var v = Browser .activeViewpoint; return v === null ? 'null' : [v .getTypeName (), v .getName ()];"),
    ("later bindViewpoint errors", "var r = []; try { Browser .bindViewpoint (null, null); } catch (e) { r .push (e .message); } try { Browser .bindViewpoint (Browser .getActiveLayer (), null); } catch (e) { r .push (e .message); } return r;"),
]


def later(probes, frame=5):
    import re
    body = "\n".join(f"  T ({lab!r}, function () {{ {code} }});" for lab, code in probes)
    return f"var F_ = 0;\nfunction prepareEvents () {{\n  if (++ F_ !== {frame}) return;\n{body}\n  flush_ ();\n}}\n"


NODES = 'DEF V1 Viewpoint { description "first" }\nDEF N1 NavigationInfo { type "EXAMINE" }\n'
PRE = CHAIN + "\n" + DESC
# Kept open (OPEN_WORK.md): the reflection of X_ITE's engine classes past the SAI edge (the Browser's ~590 internal members, the
# X3DScene and node class chains: the port hands out X3DNodeRef), and the page objects the owner decided return null (DOM, viewer,
# dashboard, StopWatch).
BOUNDARY = ("Browser chain", "getViewer ()", "getActiveViewpoint () desc", "getActiveNavigationInfo () desc", "getActiveLayer () desc",
            "getActiveViewpoint ()", "getActiveNavigationInfo ()", "getActiveLayer ()", "getBrowserOptions () desc", "getCanvas ()",
            "getElement ()", "getNotification ()", "getContextMenu ()", "getBrowserTime ()", "getBrowserTimings ()",
            "getExecutionContext", "currentScene desc")
G.register("brx-api", P_API, open_prefixes=BOUNDARY, pre=PRE, nodes=NODES)
G.register("brx-call", P_CALL, open_prefixes=BOUNDARY, pre=PRE, nodes=NODES)
G.register("brx-obj", P_OBJ, open_prefixes=BOUNDARY, pre=PRE, nodes=NODES)
G.register("brx-data", P_DATA, pre=PRE + "\n" + DEFS, after=later(P_LATER), nodes=NODES)

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n in G.defs if n.startswith("brx-")]
    G.main()
