#!/usr/bin/env python3
"""Generate the IMPORT / EXPORT chain worlds oracle/synthetic-rea-b09-sai/ie-*.wrl and their assets/ie-*.wrl (REA batch 09, step 3, item 2).
Authored; synthetic inputs only.

ie-leaf.wrl exports an interpolator and a Transform; ie-mid.wrl imports both of them from its own Inline (a nested Inline), routes the
imported interpolator and imports the leaf Transform into a Transform of its own, and exports its interpolator and Transform; the top
world imports those from ie-mid.wrl, routes into the imported interpolator, and a Script reads the values through
`importedNodes [i] .exportedNode`. X_ITE refuses to re-export an imported node ("cannot be of type X3DImportedNodeInstance"), so a chain
is always import - local node - export.

The log of the page (`log` of sai.json) carries the parse/load errors, the Script records what it saw in `hist`.
"""
import os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")
asset_dir = os.path.join(out_dir, "assets")

HEAD = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"
AHEAD = "#X3D V3.3 utf8\nPROFILE Immersive\n"


def vs(js):
    return js.replace("\\", "\\\\").replace('"', '\\"')


assets = {}

assets["ie-leaf.wrl"] = AHEAD + """DEF LI PositionInterpolator { key [0 1] keyValue [0 0 0, 10 0 0] }
DEF LT Transform { }
DEF LS Script {
  inputOnly SFVec3f set_t
  inputOutput SFString seen ""
  url "ecmascript:function set_t (v) { seen = seen + v + ' '; }"
}
ROUTE LI.value_changed TO LT.set_translation
ROUTE LT.translation_changed TO LS.set_t
EXPORT LI
EXPORT LT
EXPORT LI AS LI2
EXPORT LS
"""

assets["ie-mid.wrl"] = AHEAD + """DEF I2 Inline { url "ie-leaf.wrl" }
IMPORT I2.LI AS MLI
IMPORT I2.LT AS MLT
IMPORT I2.LS AS MLS DESCRIPTION "the leaf script"
DEF MI ScalarInterpolator { key [0 1] keyValue [0 1] }
DEF MT Transform { }
DEF MG Group { children [ DEF MC Transform { translation 5 0 0 } ] }
ROUTE MI.value_changed TO MLI.set_fraction
ROUTE MLT.translation_changed TO MT.set_translation
EXPORT MI
EXPORT MT
EXPORT MG AS MGroup
EXPORT MC
"""

TOP_ROUTES = "ROUTE TS.fraction_changed TO IMI.set_fraction\n"

COMMON = """
function p (name, f) { var v; try { v = f (); } catch (e) { v = 'ERR:' + e .message; } hist = hist + step + ':' + name + '=' + v + ' '; }
function names (s) { var r = []; for (var i = 0; i < s .importedNodes .length; ++ i) r .push (s .importedNodes [i] .importedName + '<' + s .importedNodes [i] .exportedName); return r .join (','); }
function lines (s, w) { return s .toVRMLString () .split ('\\n') .map (function (l) { return l .replace (/^\\s+/, ''); }) .filter (function (l) { return l .indexOf (w) === 0; }) .join ('|'); }
function ex (s, name) { return s .getImportedNode (name); }
"""


def world(title, top_decls, body, extra="", pre=""):
    js = COMMON + pre + f"""
function cycle () {{
  step = step + 1;
  var s = Browser .currentScene;
{body}
}}"""
    return (HEAD + f'META "title" "{title}"\n' + 'DEF TS TimeSensor { cycleInterval 1 loop TRUE }\n' + top_decls
            + extra
            + f'DEF P Script {{\n  inputOutput SFString hist ""\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n  url "ecmascript:\n{vs(js)}\n  "\n}}\n'
            + "DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }\nROUTE STEP.cycleTime TO P.cycle\n")


TOP = """DEF I1 Inline { url "assets/ie-mid.wrl" }
IMPORT I1.MI AS IMI
IMPORT I1.MT AS IMT
IMPORT I1.MGroup AS IMG
IMPORT I1.MC AS IMC DESCRIPTION "child"
"""

cases = {}

cases["ie-chain"] = world("ie chain", TOP + "ROUTE TS.fraction_changed TO IMI.set_fraction\n", """
  var o = function (n) { return s .getImportedNode (n); };
  if (step == 6) {
    p ('names', function () { return names (s); });
    p ('type', function () { return o ('IMT') .getNodeTypeName (); });
    p ('exType', function () { return s .importedNodes [1] .exportedNode .getNodeTypeName (); });
    p ('exName', function () { return s .importedNodes [1] .exportedNode .getNodeName (); });
    p ('desc', function () { return s .importedNodes [3] .description; });
    p ('routes', function () { return s .routes .length + ':' + lines (s, 'ROUTE'); });
    p ('imports', function () { return lines (s, 'IMPORT'); });
  }
  if (step == 8) p ('t8', function () { return s .importedNodes [1] .exportedNode .translation + ' ' + s .importedNodes [0] .exportedNode .value_changed; });
  if (step == 12) p ('t12', function () { return s .importedNodes [1] .exportedNode .translation; });
  if (step == 14) p ('group', function () { var g = s .importedNodes [2] .exportedNode; return g .getNodeTypeName () + ':' + g .children .length + ':' + g .children [0] .getNodeName () + ':' + g .children [0] .translation; });
  if (step == 15) p ('local', function () { return s .getLocalNode ('IMT') .getNodeTypeName () + ':' + (s .getLocalNode ('IMT') === o ('IMT')) + ':' + typeof s .getNodeOrImportedNode (s .importedNodes [1] .exportedNode); });
  if (step == 16) p ('inlineNode', function () { return s .importedNodes [0] .inlineNode .getNodeName (); });
  if (step == 17) p ('toXML', function () { return s .toXMLString () .split ('\\n') .map (function (l) { return l .replace (/^\\s+/, ''); }) .filter (function (l) { return l .indexOf ('<IMPORT') === 0; }) .join ('|'); });
""")

cases["ie-rename"] = world("ie rename", TOP + "ROUTE TS.fraction_changed TO IMI.set_fraction\n", """
  if (step == 6) {
    p ('names', function () { return names (s); });
    p ('routes0', function () { return s .routes .length; });
    p ('rename', function () { s .renameImportedNode ('IMT', 'T2'); return names (s) + ' | ' + lines (s, 'IMPORT'); });
    p ('instName', function () { return s .getImportedNode ('T2') .getNodeName () + ':' + s .getImportedNode ('T2') .getNodeDisplayName (); });
    p ('renameDup', function () { s .renameImportedNode ('T2', 'IMI'); return 'ok'; });
    p ('renameMissing', function () { s .renameImportedNode ('IMT', 'X'); return 'ok'; });
    p ('oldName', function () { return s .getImportedNode ('IMT'); });
  }
  if (step == 8) p ('t8', function () { return s .importedNodes [1] .exportedNode .translation; });
  if (step == 9) {
    p ('update', function () { s .updateImportedNode (s .getNamedNode ('I1'), 'MC', 'T2'); return names (s) + ' | ' + s .getImportedNode ('T2') .getNodeTypeName (); });
    p ('updateExported', function () { return s .importedNodes [1] .exportedNode .getNodeName (); });
    p ('routes1', function () { return s .routes .length + ':' + lines (s, 'ROUTE'); });
  }
  if (step == 10) {
    p ('addRouteImp', function () { s .addRoute (s .getNamedNode ('TS'), 'fraction_changed', s .getImportedNode ('T2'), 'set_scale'); return s .routes .length + ':' + lines (s, 'ROUTE'); });
    p ('addRouteBad', function () { s .addRoute (s .getNamedNode ('TS'), 'fraction_changed', s .getImportedNode ('T2'), 'nope'); return s .routes .length; });
  }
  if (step == 11) p ('removeImp', function () { s .removeImportedNode ('T2'); return names (s) + ' | routes ' + s .routes .length + ':' + lines (s, 'ROUTE'); });
  if (step == 12) p ('removeImp2', function () { s .removeImportedNode ('IMI'); return names (s) + ' | routes ' + s .routes .length; });
  if (step == 13) p ('addBack', function () { s .addImportedNode (s .getNamedNode ('I1'), 'MI', 'IMI'); return names (s); });
  if (step == 14) p ('addMissingExport', function () { s .addImportedNode (s .getNamedNode ('I1'), 'NOPE', 'IMN'); return names (s) + ' | ' + lines (s, 'IMPORT'); });
  if (step == 15) p ('missingExported', function () { return s .importedNodes [s .importedNodes .length - 1] .exportedNode .getNodeName (); });
  if (step == 16) p ('routeToMissing', function () { s .addRoute (s .getNamedNode ('TS'), 'fraction_changed', s .getImportedNode ('IMN'), 'set_fraction'); return s .routes .length; });
  if (step == 17) p ('vrml', function () { return lines (s, 'IMPORT') + ' ## ' + lines (s, 'ROUTE'); });
""")


# A second Inline of the same file next to the chain; routes in both directions across imported nodes, writes through the exported node.
DIRECT = TOP + """DEF IL Inline { url "assets/ie-leaf.wrl" }
IMPORT IL.LI AS TLI
IMPORT IL.LT AS TLT
IMPORT IL.LS AS TLS
IMPORT IL.LI2 AS TLI2
DEF TOPT Transform { }
ROUTE TS.fraction_changed TO IMI.set_fraction
ROUTE TS.fraction_changed TO TLI.set_fraction
ROUTE TLT.translation_changed TO TOPT.set_translation
"""
cases["ie-direct"] = world("ie direct", DIRECT, """
  var ex = function (n) { return s .getImportedNode (n); };
  if (step == 6) {
    p ('names', function () { return names (s); });
    p ('same', function () { return s .importedNodes [4] .exportedNode .getNodeName () + ':' + s .importedNodes [7] .exportedNode .getNodeName () + ':' + (s .importedNodes [4] .exportedNode === s .importedNodes [7] .exportedNode); });
    p ('routes', function () { return s .routes .length + ':' + lines (s, 'ROUTE'); });
  }
  if (step == 8) p ('top', function () { return s .getNamedNode ('TOPT') .translation + ' ' + s .importedNodes [5] .exportedNode .translation + ' ' + s .importedNodes [6] .exportedNode .seen .length; });
  if (step == 10) p ('write', function () { var li = s .importedNodes [4] .exportedNode; li .set_fraction = 0.5; return li .value_changed + ' ' + s .getNamedNode ('TOPT') .translation; });
  if (step == 11) p ('writeScript', function () { s .importedNodes [6] .exportedNode .seen = 'x'; return s .importedNodes [6] .exportedNode .seen; });
  if (step == 12) p ('deleteRoute', function () { s .deleteRoute (s .getNamedNode ('TS'), 'fraction_changed', s .getImportedNode ('TLI'), 'set_fraction'); return s .routes .length; });
  if (step == 14) p ('after', function () { return s .getNamedNode ('TOPT') .translation; });
  if (step == 15) p ('removeInline', function () { s .removeImportedNode ('TLT'); return s .routes .length + ' ' + names (s); });
  if (step == 16) p ('type', function () { return s .getImportedNode ('TLS') .getNodeTypeName () + ':' + s .getImportedNode ('TLI2') .getNodeName () + ':' + s .getImportedNode ('TLI2') .getNodeDisplayName (); });
  if (step == 17) p ('uniq', function () { return s .getUniqueImportName ('TLI') + ':' + s .getUniqueName ('TOPT') + ':' + s .getUniqueName ('IL'); });
""")

# Load-time errors: the log of the page.
errs = {
    "ie-err-missing": ('DEF I1 Inline { url "assets/ie-leaf.wrl" }\nIMPORT I1.NOPE AS X\nROUTE TS.fraction_changed TO X.set_fraction\n', {}),
    "ie-err-dupas": ('DEF I1 Inline { url "assets/ie-leaf.wrl" }\nIMPORT I1.LI AS X\nIMPORT I1.LT AS X\n', {}),
    "ie-err-dupname": ('DEF I1 Inline { url "assets/ie-leaf.wrl" }\nDEF X Transform { }\nIMPORT I1.LI AS X\n', {}),
    "ie-err-export-dup": ('DEF I1 Inline { url "assets/ie-dupexp.wrl" }\nIMPORT I1.A AS X\n', {"ie-dupexp.wrl": AHEAD + "DEF A Transform { }\nDEF B Transform { }\nEXPORT A\nEXPORT B AS A\n"}),
    "ie-err-export-missing": ('DEF I1 Inline { url "assets/ie-expmissing.wrl" }\nIMPORT I1.A AS X\n', {"ie-expmissing.wrl": AHEAD + "DEF A Transform { }\nEXPORT NOPE\n"}),
    "ie-err-export-imported": ('DEF I1 Inline { url "assets/ie-expimp.wrl" }\nIMPORT I1.A AS X\n', {"ie-expimp.wrl": AHEAD + "DEF I Inline { url \"ie-leaf.wrl\" }\nIMPORT I.LI AS A\nEXPORT A\n"}),
    "ie-err-self": ('DEF I1 Inline { url "assets/ie-self.wrl" }\nIMPORT I1.A AS X\nROUTE X.translation_changed TO TS.set_cycleInterval\n', {"ie-self.wrl": AHEAD + "DEF A Transform { }\nEXPORT A\nEXPORT A AS B\nEXPORT A AS B\n"}),
    "ie-err-route-before": ('DEF I1 Inline { url "assets/ie-leaf.wrl" }\nIMPORT I1.LI AS X\nROUTE TS.fraction_changed TO X.nofield\nROUTE X.value_changed TO TS.set_cycleInterval\n', {}),
    "ie-err-load-failed": ('DEF I1 Inline { url "assets/ie-missing-file.wrl" }\nIMPORT I1.LI AS X\nROUTE TS.fraction_changed TO X.set_fraction\n', {}),
}

# Scenes that fail to parse: the rejection of createX3DFromString carries the message.
STRINGS = [
    "DEF T Transform { }\nIMPORT T.LI AS X\n",
    "IMPORT NOSUCH.LI AS X\n",
    "DEF I Inline { }\nIMPORT I.A AS X\nIMPORT I.B AS X\n",
    "DEF I Inline { }\nEXPORT NOSUCH\n",
    "DEF T Transform { }\nEXPORT T\nEXPORT T AS T\n",
    "DEF T Transform { }\nDEF U Transform { }\nEXPORT T AS Q\nEXPORT U AS Q\n",
    "DEF I Inline { }\nIMPORT I.A AS X\nEXPORT X\n",
    "DEF I Inline { }\nIMPORT I.A\nIMPORT I.A AS B\n",
    "DEF I Inline { }\nIMPORT I\n",
    "DEF I Inline { }\nIMPORT I.\n",
    "DEF I Inline { }\nIMPORT .A\n",
    "DEF I Inline { }\nIMPORT I.A AS\n",
    "EXPORT\n",
    "DEF T Transform { }\nEXPORT T AS\n",
    "DEF T Transform { }\nEXPORT T\nROUTE T.translation_changed TO T.set_translation\n",
]
for name, (decls, extra_assets) in errs.items():
    cases[name] = world(name.replace("-", " "), decls, """
  if (step == 8) { p ('names', function () { return names (s); }); p ('routes', function () { return s .routes .length; });
    p ('ex', function () { return s .importedNodes .length ? s .importedNodes [0] .exportedNode .getNodeName () : 'none'; }); }
""")
    assets.update(extra_assets)

import json

cases["ie-err-string"] = world("ie err string", "", """
  // The promises settle a frame apart in the two browsers (see REPORT), so the results are collected and shown at the end.
  if (step >= 3 && step <= TEXTS .length + 2) {
    var i = step - 3;
    Browser .createX3DFromString (TEXTS [i]) .then (
      function (sc) { results [i] = i + ':ok ' + sc .importedNodes .length + '/' + sc .exportedNodes .length; },
      function (e) { results [i] = i + ':ERR ' + String (e && e .message) .replace (/http:\\/\\/[^\\/]+/g, 'H') .split ('\\n') .join ('|'); });
  }
  if (step == TEXTS .length + 6) hist = results .join (' ');
""", pre="var TEXTS = " + json.dumps(STRINGS) + ";\nvar results = [];\n")

# The same texts, each result recorded in `hist` the moment its promise settles. The parsers run their statements after
# `browser .loadComponents (scene)` resolves (a microtask, after the frame), so a scene requested during a frame resolves one frame
# later than a rejection.
cases["ie-err-string-frames"] = world("ie err string frames", "", """
  if (step >= 3 && step <= TEXTS .length + 2) {
    var i = step - 3;
    Browser .createX3DFromString (TEXTS [i]) .then (
      function (sc) { hist = hist + i + ':ok '; },
      function (e) { hist = hist + i + ':ERR '; });
  }
""", pre="var TEXTS = " + json.dumps(STRINGS) + ";\n")

if __name__ == "__main__":
    os.makedirs(asset_dir, exist_ok=True)

    for name, text in cases.items():
        with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
            f.write(text)

    for name, text in assets.items():
        with open(os.path.join(asset_dir, name), "w") as f:
            f.write(text)

    print(len(cases), "worlds")
