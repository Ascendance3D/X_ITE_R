#!/usr/bin/env python3
"""Generate the execution-context change-event worlds oracle/synthetic-rea-b09-sai/ctxev-*.wrl (REA batch 09, step 3, item 1).
Authored; synthetic inputs only.

Pinned X_ITE 16.5.0: X3DExecutionContext has exactly one observable change field, `sceneGraph_changed` (SFTime, set to the real clock,
also set by every node `parentsChanged`, and forwarded from nested contexts); X3DScene adds `profile_changed`, `metadata_changed`
(`bbox_changed` is on the context too). `_namedNodes_changed` is a plain property assigned a number, not a field; `_importedNodes_changed`,
`_exportedNodes_changed`, `_protos_changed`, `_externprotos_changed`, `_rootNodes_changed`, `_routes_changed` do not exist at all.
So the probes compare only: that a field callback fires, in which frame (the Script's `hist` field is dumped at every frame), in what order
relative to the other records, and typeof of the plain properties. The clock value itself is never recorded.

Each case: the Script P registers addFieldCallback on the sceneGraph_changed/profile_changed/metadata_changed fields of the scene, then
runs one action per cycle step and records `step:action=result`.
"""
import os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")

HEAD = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"


def vs(js):
    return js.replace("\\", "\\\\").replace('"', '\\"')


# Callback keys are distinct per field; every callback appends `<tag>@<step>` to hist (gated: nothing before step 2, so the start-up
# bursts of sceneGraph_changed are probed by the ctxev-startup-* cases only).
COMMON = """
function rec (s) { hist = hist + s + ' '; }
function recg (s) { if (step >= 2) rec (s); }
function p (name, f) {
  var v;
  try { v = f (); } catch (e) { v = 'ERR:' + e .message; }
  rec (step + ':' + name + '=' + v);
}
function types (s) {
  return ['namedNodes', 'importedNodes', 'exportedNodes', 'protos', 'externprotos', 'rootNodes', 'routes', 'sceneGraph', 'profile', 'metadata', 'bbox']
    .map (function (n) { return n + ':' + typeof s ['_' + n + '_changed'] + '/' + typeof s [n + '_changed']; }) .join (',');
}
function watch (s, label, gate) {
  var r = gate ? recg : rec;
  s .sceneGraph_changed .addFieldCallback ('sg' + label, function () { r ('SG' + label + '@' + step); });
  s .profile_changed  .addFieldCallback ('pr' + label, function () { r ('PR' + label + '@' + step); });
  s .metadata_changed .addFieldCallback ('md' + label, function () { r ('MD' + label + '@' + step); });
}
"""


def world(title, body, pre="", nodes="", decls="", init="", gate=True):
    js = f"""{COMMON}
{pre}
function initialize () {{
  var s = Browser .currentScene;
{init}
  watch (s, '', {'true' if gate else 'false'});
}}
function cycle () {{
  step = step + 1;
  var s = Browser .currentScene;
{body}
}}"""
    return (HEAD + f'META "title" "{title}"\n' + decls + "DEF T1 Transform { }\nDEF T2 Transform { translation 100 0 0 }\nDEF T3 Transform { }\nDEF T4 Transform { }\n" + nodes
            + f'DEF P Script {{\n  inputOutput SFString hist ""\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n  url "ecmascript:\n{vs(js)}\n  "\n}}\n'
            + "DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }\nROUTE STEP.cycleTime TO P.cycle\n")


cases = {}

cases["ctxev-named"] = world("ctxev named", """
  if (step == 1) p ('types', function () { return types (s); });
  if (step == 2) p ('updNew', function () { s .updateNamedNode ('Q', s .getNamedNode ('T1')); return s .namedNodes .length; });
  if (step == 3) p ('updRename', function () { s .updateNamedNode ('Q', s .getNamedNode ('T2')); return s .namedNodes .length + ':' + s .getNamedNode ('Q') .getNodeName () + ':' + s .getNamedNode ('T1') .getNodeName (); });
  if (step == 4) p ('remove', function () { s .removeNamedNode ('Q'); return s .namedNodes .length; });
  if (step == 5) p ('addNew', function () { s .addNamedNode ('R', s .getNamedNode ('T3')); return s .namedNodes .length; });
  if (step == 6) p ('addDup', function () { s .addNamedNode ('R', s .getNamedNode ('T4')); return s .namedNodes .length; });
  if (step == 7) p ('removeMissing', function () { s .removeNamedNode ('NOPE'); return s .namedNodes .length; });
  if (step == 8) p ('removeR', function () { s .removeNamedNode ('R'); return s .namedNodes .length; });
  if (step == 9) p ('types', function () { return types (s); });
""")

cases["ctxev-imported"] = world("ctxev imported", """
  var inl = s .getNamedNode ('INL'), inl2 = s .getNamedNode ('INL2');
  if (step == 2) p ('add', function () { s .addImportedNode (inl, 'E1', 'IM1'); return s .importedNodes .length; });
  if (step == 3) p ('addDup', function () { s .addImportedNode (inl, 'E1', 'IM1'); return s .importedNodes .length; });
  if (step == 4) p ('update', function () { s .updateImportedNode (inl2, 'E3', 'IM1'); return s .importedNodes .length; });
  if (step == 5) p ('rename', function () { s .renameImportedNode ('IM1', 'IM2'); return s .importedNodes [0] .importedName; });
  if (step == 6) p ('renameMissing', function () { s .renameImportedNode ('NOPE', 'IM3'); return 'ok'; });
  if (step == 7) p ('remove', function () { s .removeImportedNode ('IM2'); return s .importedNodes .length; });
  if (step == 8) p ('removeMissing', function () { s .removeImportedNode ('IM2'); return s .importedNodes .length; });
""", nodes='DEF INL Inline { url "assets/exp.wrl" }\nDEF INL2 Inline { url "assets/exp.wrl" }\n')

cases["ctxev-exported"] = world("ctxev exported", """
  if (step == 2) p ('add', function () { s .addExportedNode ('X1', s .getNamedNode ('T1')); return s .exportedNodes .length; });
  if (step == 3) p ('addDup', function () { s .addExportedNode ('X1', s .getNamedNode ('T2')); return s .exportedNodes .length; });
  if (step == 4) p ('update', function () { s .updateExportedNode ('X1', s .getNamedNode ('T2')); return s .exportedNodes .length + ':' + s .getExportedNode ('X1') .getNodeName (); });
  if (step == 5) p ('updateNew', function () { s .updateExportedNode ('X2', s .getNamedNode ('T1'), 'd'); return s .exportedNodes .length; });
  if (step == 6) p ('remove', function () { s .removeExportedNode ('X1'); return s .exportedNodes .length; });
  if (step == 7) p ('removeMissing', function () { s .removeExportedNode ('X1'); return s .exportedNodes .length; });
  if (step == 8) p ('removeX2', function () { s .removeExportedNode ('X2'); return s .exportedNodes .length; });
""")

cases["ctxev-protos"] = world("ctxev protos", """
  if (step == 2) p ('removeProto', function () { s .removeProtoDeclaration ('PA'); return s .protos .length; });
  if (step == 3) p ('addProto', function () { s .addProtoDeclaration ('PA', saved); return s .protos .length; });
  if (step == 4) p ('updateProto', function () { s .updateProtoDeclaration ('PC', saved); return s .protos .length + ':' + s .protos [0] .name + ',' + s .protos [1] .name; });
  if (step == 5) p ('addDupProto', function () { s .addProtoDeclaration ('PA', saved); return s .protos .length; });
  if (step == 6) p ('removeExtern', function () { s .removeExternProtoDeclaration ('EP'); return s .externprotos .length; });
  if (step == 7) p ('addExtern', function () { s .addExternProtoDeclaration ('EP', savedE); return s .externprotos .length; });
  if (step == 8) p ('updateExtern', function () { s .updateExternProtoDeclaration ('EP2', savedE); return s .externprotos .length; });
  if (step == 9) p ('removeMissing', function () { s .removeProtoDeclaration ('NOPE'); s .removeExternProtoDeclaration ('NOPE'); return s .protos .length + ':' + s .externprotos .length; });
""", pre="var saved = null, savedE = null;", init="  saved = s .getProtoDeclaration ('PA');\n  savedE = s .getExternProtoDeclaration ('EP');",
    decls='EXTERNPROTO EP [ inputOutput SFFloat v ] [ "missing-ep.wrl#EP" ]\nPROTO PA [ inputOutput SFVec3f p 1 2 3 ] { Group { } }\nPROTO PB [ ] { Group { } }\n')

cases["ctxev-routes"] = world("ctxev routes", """
  var ts = s .getNamedNode ('TS');
  if (step == 2) p ('add', function () { s .addRoute (ts, 'fraction_changed', s .getNamedNode ('CI'), 'set_fraction'); return s .routes .length; });
  if (step == 3) p ('addDup', function () { s .addRoute (ts, 'fraction_changed', s .getNamedNode ('CI'), 'set_fraction'); return s .routes .length; });
  if (step == 4) p ('addOther', function () { s .addRoute (s .getNamedNode ('T1'), 'translation_changed', s .getNamedNode ('T2'), 'translation'); return s .routes .length; });
  if (step == 5) p ('delete4', function () { s .deleteRoute (ts, 'fraction_changed', s .getNamedNode ('CI'), 'set_fraction'); return s .routes .length; });
  if (step == 6) p ('deleteObj', function () { s .deleteRoute (s .routes [s .routes .length - 1]); return s .routes .length; });
  if (step == 7) p ('deleteMissing', function () { s .deleteRoute (ts, 'fraction_changed', s .getNamedNode ('CI'), 'set_fraction'); return s .routes .length; });
  if (step == 8) p ('routeFromScene', function () { s .addRoute (s, 'sceneGraph_changed', s .getNamedNode ('CI'), 'set_fraction'); return s .routes .length; });
  if (step == 9) p ('routeToScene', function () { s .addRoute (s .getNamedNode ('TS'), 'fraction_changed', s, 'sceneGraph_changed'); return s .routes .length; });
  if (step == 10) p ('getFieldScene', function () { return s .getField ('sceneGraph_changed'); });
""", nodes='DEF TS TimeSensor { }\nDEF CI PositionInterpolator { key [0 1] keyValue [0 0 0, 1 1 1] }\n')

cases["ctxev-roots"] = world("ctxev roots", """
  var me = s .getNamedNode ('P');
  if (step == 2) p ('setRoots', function () { s .rootNodes = new MFNode (me, s .getNamedNode ('STEP'), s .getNamedNode ('T1')); return s .rootNodes .length; });
  if (step == 3) p ('setRootsSame', function () { s .rootNodes = new MFNode (me, s .getNamedNode ('STEP'), s .getNamedNode ('T1')); return s .rootNodes .length; });
  if (step == 4) p ('addRoot', function () { s .addRootNode (s .getNamedNode ('T2')); return s .rootNodes .length; });
  if (step == 5) p ('removeRoot', function () { s .removeRootNode (s .getNamedNode ('T2')); return s .rootNodes .length; });
  if (step == 6) p ('createNode', function () { var n = s .createNode ('Transform'); return n .getNodeTypeName (); });
  if (step == 7) p ('createAdd', function () { var n = s .createNode ('Transform'); s .addRootNode (n); return s .rootNodes .length; });
  if (step == 8) p ('fieldAssign', function () { var g = s .createNode ('Group'); g .children = new MFNode (s .getNamedNode ('T1')); return 'ok'; });
  if (step == 9) p ('setReduced', function () { s .rootNodes = new MFNode (me, s .getNamedNode ('STEP')); return s .rootNodes .length; });
""")

cases["ctxev-meta"] = world("ctxev meta", """
  if (step == 2) p ('setMeta', function () { s .setMetaData ('k', 'v'); return s .getMetaData ('k'); });
  if (step == 3) p ('addMeta', function () { s .addMetaData ('k', 'w'); return s .getMetaData ('k'); });
  if (step == 4) p ('removeMeta', function () { s .removeMetaData ('k'); return String (s .getMetaData ('k')); });
  if (step == 5) p ('setProfile', function () { s .setProfile (Browser .getProfile ('Interchange')); return s .profile .name; });
  if (step == 6) p ('addComponent', function () { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .components .length; });
  if (step == 7) p ('setUnit', function () { s .updateUnit ('length', 'foot', 0.3048); return s .units .length; });
""")

cases["ctxev-inline"] = world("ctxev inline", """
  if (step == 4) p ('set', function () { s .getNamedNode ('INL') .url = new MFString ('assets/lu-inl.wrl'); return 'ok'; });
  if (step == 9) p ('loadFalse', function () { s .getNamedNode ('INL') .load = false; return 'ok'; });
  if (step == 11) p ('loadTrue', function () { s .getNamedNode ('INL') .load = true; return 'ok'; });
""", nodes='DEF INL Inline { url "assets/exp.wrl" }\n')

# Scenes that are not the world: createScene () / createX3DFromString (): the callbacks of their own contexts.
cases["ctxev-scene"] = world("ctxev scene", """
  if (step == 2) p ('make', function () { Browser .createScene (Browser .getProfile ('Full')) .then (function (sc) { made = sc; watch (sc, 'M', false); }); return 'started'; });
  if (step == 3) p ('makeStr', function () { Browser .createX3DFromString ('#X3D V3.3 utf8\\nPROFILE Full\\nDEF A Transform { }\\nDEF B Transform { }\\nPROTO PP [ ] { Group { } }') .then (function (sc) { made2 = sc; watch (sc, 'S', false); }); return 'started'; });
  if (step == 7) p ('rootsM', function () { made .addRootNode (made .createNode ('Transform')); return made .rootNodes .length; });
  if (step == 8) p ('rootsM2', function () { made .rootNodes = new MFNode (made .createNode ('Group'), made .createNode ('Group')); return made .rootNodes .length; });
  if (step == 9) p ('createM', function () { made .createNode ('Transform'); return 'x'; });
  if (step == 10) p ('namedM', function () { made .updateNamedNode ('Q', made .rootNodes [0]); return 'x'; });
  if (step == 11) p ('metaM', function () { made .setMetaData ('a', 'b'); made .setProfile (Browser .getProfile ('Core')); return 'x'; });
  if (step == 12) p ('rootsS', function () { made2 .addRootNode (made2 .createNode ('Transform')); return made2 .rootNodes .length; });
  if (step == 13) p ('removeS', function () { made2 .removeRootNode (made2 .rootNodes [0]); return made2 .rootNodes .length; });
  if (step == 14) p ('protoS', function () { made2 .removeProtoDeclaration ('PP'); return made2 .protos .length; });
  if (step == 15) p ('exportS', function () { made2 .updateExportedNode ('E', made2 .getNamedNode ('A')); return 'x'; });
  if (step == 16) p ('createProto', function () { var pr = made2 .createProto ('PP'); return pr .getNodeTypeName (); });
  if (step == 17) p ('worldRoots', function () { s .addRootNode (s .createNode ('Group')); return 'x'; });
  if (step == 18) p ('worldCreateProto', function () { return 'skip'; });
""", pre="var made = null, made2 = null;")

# Start-up: every callback, from initialize (), on worlds of different shapes.
for n, nodes, decls in [("plain", "", ""), ("inline", 'DEF INL Inline { url "assets/exp.wrl" }\n', ""),
                        ("proto", "DEF PI PA { }\n", "PROTO PA [ ] { Group { children Transform { } } }\n"),
                        ("nested", 'DEF INL Inline { url "assets/nested.wrl" }\n', "")]:
    cases["ctxev-startup-" + n] = world("ctxev startup " + n, "  if (step == 1) p ('t', function () { return 'x'; });\n", nodes=nodes, decls=decls, gate=False)

# World switches. A's callbacks log to the console (A's world is gone after the switch); B records in its own hist.
B_WORLD = HEAD + 'DEF VPB Viewpoint { description "vpB" }\nDEF TB Transform { }\n' + (
    'DEF BS Script {\n  inputOutput SFString hist ""\n  inputOutput SFInt32 step 0\n  url "ecmascript:\n'
    + vs("""
function rec (s) { hist = hist + s + ' '; }
function initialize () {
  var s = Browser .currentScene;
  rec ('init');
  s .sceneGraph_changed .addFieldCallback ('b', function () { rec ('SGB'); });
  s .profile_changed .addFieldCallback ('b', function () { rec ('PRB'); });
  s .metadata_changed .addFieldCallback ('b', function () { rec ('MDB'); });
}""") + '\n  "\n}\n')

B_WORLD_META = B_WORLD.replace("COMPONENT Scripting:1\n", "COMPONENT Scripting:1\nUNIT length foot 0.3048\nMETA \"k\" \"v\"\n", 1)
assert B_WORLD_META != B_WORLD

A_SW = """
  if (step == 1) { var cs = Browser .currentScene;
    cs .sceneGraph_changed .addFieldCallback ('a', function () { console .log ('A: SG@' + step + ' ' + (Browser .currentScene === cs ? 'cur' : 'old')); }); }
  if (step == 2) p ('act', function () { @ACT@ });
"""
cases["ctxev-loadurl"] = world("ctxev loadurl", A_SW.replace("@ACT@", "Browser .loadURL (new MFString ('assets/ce-b.wrl')) .then (function () { console .log ('A: resolved@' + step); }); return 'started';"), gate=True)
cases["ctxev-loadurl-meta"] = world("ctxev loadurl meta", A_SW.replace("@ACT@", "Browser .loadURL (new MFString ('assets/ce-b-meta.wrl')) .then (function () { console .log ('A: resolved@' + step); }); return 'started';"), gate=True)
cases["ctxev-replace"] = world("ctxev replace", A_SW.replace("@ACT@", "Browser .replaceWorld (null); return 'ok';"), gate=True)

# A scene's toVRMLString / toXMLString / toJSONString reads the root nodes of every PROTO declaration body through the MFNode proxy;
# the first read makes their SFNodes (SFNodeCache), whose addParent is a parentsChanged of the body: sceneGraph_changed of the scene.
# The worlds are dumped at the last frame only (sai-last.json), so the page's own toVRMLString does not read them first.
STREAM_DECLS = ("PROTO PA [ ] { Group { children Transform { } } }\n"
                "PROTO PB [ ] { PROTO PC [ ] { Transform { } } Group { } }\n"
                "PROTO PE [ ] { }\n")
for fmt in ("VRML", "XML", "JSON"):
    cases["ctxev-stream-" + fmt.lower()] = world("ctxev stream " + fmt.lower(), f"""
  if (step == 4) p ('first', function () {{ return s .to{fmt}String () .length > 0; }});
  if (step == 8) p ('again', function () {{ return s .to{fmt}String () .length > 0; }});
""", nodes="DEF PI PA { }\n", decls=STREAM_DECLS)

if __name__ == "__main__":
    os.makedirs(os.path.join(out_dir, "assets"), exist_ok=True)

    for name, text in cases.items():
        with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
            f.write(text)

    with open(os.path.join(out_dir, "assets", "ce-b.wrl"), "w") as f:
        f.write(B_WORLD)

    with open(os.path.join(out_dir, "assets", "ce-b-meta.wrl"), "w") as f:
        f.write(B_WORLD_META)

    print(len(cases), "worlds")
