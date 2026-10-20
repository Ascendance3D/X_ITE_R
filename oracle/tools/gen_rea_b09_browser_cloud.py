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
# node class chains: the port hands out X3DNodeRef), and the page objects the owner decided return null (DOM, viewer,
# dashboard, StopWatch).
BOUNDARY = ("Browser chain", "getViewer ()", "getActiveViewpoint () desc", "getActiveNavigationInfo () desc", "getActiveLayer () desc",
            "getActiveViewpoint ()", "getActiveNavigationInfo ()", "getActiveLayer ()", "getBrowserOptions () desc", "getCanvas ()",
            "getElement ()", "getNotification ()", "getContextMenu ()", "getBrowserTime ()", "getBrowserTimings ()")
G.register("brx-api", P_API, open_prefixes=BOUNDARY, pre=PRE, nodes=NODES)
G.register("brx-call", P_CALL, open_prefixes=BOUNDARY, pre=PRE, nodes=NODES)
G.register("brx-obj", P_OBJ, open_prefixes=BOUNDARY, pre=PRE, nodes=NODES)
G.register("brx-data", P_DATA, pre=PRE + "\n" + DEFS, after=later(P_LATER), nodes=NODES)


# The scene objects a Script gets (X3DScene → X3DExecutionContext → X3DBaseNode → X3DChildObject → X3DObject): for each public
# member, the class that owns it and its kind (m method, g getter, gs getter + setter, v value; + enumerable), and what each kind
# of scene is (the world being initialized, the first empty world, createScene, createX3DFromString).
SCX_NAMES = ("addComponent addExportedNode addExternProtoDeclaration addImportedNode addMetaData addNamedNode addProtoDeclaration "
             "addRootNode addRoute baseURL bbox_changed components createNode createProto deleteRoute encoding exportedNodes "
             "externprotos fromUnit getBaseURL getComponents getEncoding getExportedNode getExportedNodes getExternProtoDeclaration "
             "getExternProtoDeclarations getField getFieldDefinitions getImportedNode getImportedNodes getLocalNode getMetaData "
             "getMetaDatas getNamedNode getNamedNodes getNodeOrImportedNode getProfile getProtoDeclaration getProtoDeclarations "
             "getRootNodes getRoute getRoutes getSpecificationVersion getTypeName getUniqueExportName getUniqueExternProtoName "
             "getUniqueImportName getUniqueName getUniqueProtoName getUnit getUnits getWorldInfos getWorldURL hasComponent "
             "importedNodes isScene metadata_changed namedNodes profile profile_changed protos removeComponent removeExportedNode "
             "removeExternProtoDeclaration removeImportedNode removeMetaData removeNamedNode removeProtoDeclaration removeRootNode "
             "renameImportedNode rootNodes routes sceneGraph_changed setMetaData setProfile setRootNodes specificationVersion "
             "toJSONString toUnit toVRMLString toXMLString units updateComponent updateExportedNode updateExternProtoDeclaration "
             "updateImportedNode updateNamedNode updateProtoDeclaration updateUnit worldURL getType getName getId getUserData "
             "setUserData removeUserData addInterest removeInterest getExecutionContext getBrowser isLive getLive toString "
             "toVRMLStream toXMLStream toJSONStream dispose getFields getUserDefinedFields getPredefinedFields addEvent "
             "isInitialized setup initialize getMainScene getLocalScene getOuterNode isRootContext getComponentName getContainerField "
             "addChildObjects getUnitInfo").split()
SCX = ("var SCX_NAMES = " + repr(SCX_NAMES) + ";\n"
       "function owner (o, n) { var p = o, k = 0; while (p && k ++ < 10) { var d = Object .getOwnPropertyDescriptor (p, n); "
       "if (d) return (p === o ? 'OWN' : p .constructor && Object .prototype .hasOwnProperty .call (p, 'constructor') ? p .constructor .name : '-') + ':' "
       "+ ('value' in d ? (typeof d .value === 'function' ? 'm' : 'v') : (d .get ? 'g' : '') + (d .set ? 's' : '')) + (d .enumerable ? '+' : ''); "
       "p = Object .getPrototypeOf (p); } return 'none'; }\n"
       "function owners (o) { return SCX_NAMES .map (function (n) { return n + '=' + owner (o, n); }); }\n"
       "var MADE = null, STR = null;\n"
       "var MADE_ERR = '';\n"
       "Browser .createScene (Browser .getProfile ('Core'), Browser .getComponent ('Grouping', 1)) .then (function (s) { MADE = s; }, function (e) { MADE_ERR = String (e && e .message); });\n"
       "Browser .createX3DFromString ('#X3D V3.3 utf8\\nPROFILE Core\\nDEF T Transform { }') .then (function (s) { STR = s; });\n")
P_SCX = [
    ("world desc", "return desc (Browser .currentScene);"),
    ("world owners", "return owners (Browser .currentScene);"),
    ("first desc", "return desc (Browser .getExecutionContext ());"),
    ("first owners", "return owners (Browser .getExecutionContext ());"),
    ("world chain", "return chain (Browser .currentScene);"),
]
P_SCX_LATER = [
    ("later world desc", "return desc (Browser .currentScene);"),
    ("later made error", "return MADE_ERR;"),
    ("later made desc", "return desc (MADE);"),
    ("later made owners", "return owners (MADE);"),
    ("later string desc", "return desc (STR);"),
    ("later string owners", "return owners (STR);"),
    ("later made === string", "return [MADE === STR, MADE .constructor === STR .constructor, Object .getPrototypeOf (MADE) === Object .getPrototypeOf (Browser .currentScene)];"),
    ("later typeof globals", "return [typeof X3DScene, typeof X3DExecutionContext, typeof X3DBaseNode];"),
    ("later string desc after DEF", "STR .addNamedNode ('Q', STR .createNode ('Group')); return desc (STR);"),
    ("later made desc after comp", "MADE .removeComponent ('Nope'); return desc (MADE);"),
]
# What the members do on a scene object (the world's, the string scene's, the createScene scene).
P_SCX_BEHAVIOR = []
for who in ["Browser .currentScene", "STR", "MADE"]:
    P_SCX_BEHAVIOR += [
        (f"{who} names", f"var o = {who}; return [o .getTypeName (), String (o), o .toString (), Object .prototype .toString .call (o), o .getName (), typeof o .getId ()];"),
        (f"{who} getType", f"return Array .from ({who} .getType ());"),
        (f"{who} getExecutionContext", f"var c = {who} .getExecutionContext (); return c === null ? 'null' : [c === Browser .currentScene, c .getTypeName ()];"),
        (f"{who} getLocalScene", f"return {who} .getLocalScene () === {who};"),
        (f"{who} getOuterNode", f"return String ({who} .getOuterNode ());"),
        (f"{who} getField", f"try {{ return {who} .getField ('rootNodes'); }} catch (e) {{ return e .message; }}"),
        (f"{who} getFieldDefinitions", f"return {who} .getFieldDefinitions () .length;"),
        (f"{who} fields", f"return [{who} .getFields () .length, {who} .getUserDefinedFields () .length, {who} .getPredefinedFields () .length];"),
        (f"{who} toVRMLString", f"return {who} .toVRMLString () .split ('\\n') .length;"),
        (f"{who} instanceof", f"var o = {who}; return [o instanceof X3DScene, o instanceof X3DExecutionContext, o instanceof X3DBaseNode, o instanceof X3DObject];"),
        (f"{who} isScene", f"return [{who} .isScene, Object .prototype .hasOwnProperty .call ({who}, 'isScene')];"),
    ]
P_SCX_BEHAVIOR += [
    ("classes", "return [X3DScene .length, X3DScene .typeName, X3DExecutionContext .length, X3DExecutionContext .typeName, X3DScene .name, Object .getPrototypeOf (X3DScene .prototype) === X3DExecutionContext .prototype];"),
    ("method lengths", "var p = X3DScene .prototype, q = X3DExecutionContext .prototype; return ['addComponent', 'setProfile', 'fromUnit', 'updateUnit', 'addMetaData', 'addRootNode', 'toVRMLStream'] .map (function (n) { return n + p [n] .length; }) .concat (['addRoute', 'deleteRoute', 'getRoute', 'addImportedNode', 'updateImportedNode', 'createNode', 'getUniqueName'] .map (function (n) { return n + q [n] .length; }));"),
]
P_SCX_LATER += P_SCX_BEHAVIOR
G.register("scx-api", P_SCX, open_prefixes=("world chain",), pre=PRE + "\n" + SCX, after=later(P_SCX_LATER), nodes=NODES)

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n in G.defs if n.startswith(("brx-", "scx-"))]
    G.main()
