#!/usr/bin/env python3
"""Generate the node field and event survey worlds oracle/synthetic-rea-b09-sai/node-*.wrl (REA batch 09 cloud: the NOT_TESTED nodes).
Authored; synthetic inputs only. Uses the probe harness of gen_rea_b09_script.py (`T (label, fn)` into the MFString `out`).

For every node type of a group the pinned X_ITE is asked, from a Script:
  * its field definitions (name, access type, data type, the default value's text) and a created node's VRML, XML and JSON text;
  * on a node of the scene (a live instance, held in the Script's `keep` field): a field callback on every field that sends events;
    at frame 3 every inputOnly and inputOutput field that is not a node field is given a sample value of its type (an SFBool the
    opposite of its value); at frame 6 the callbacks that ran (in order, with the value's text) and the instance's VRML text.
Visual evidence is not part of these worlds (no rendering is compared by the SAI lane).

  python3 -I oracle/tools/gen_rea_b09_nodes_cloud.py
"""
import json, os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_rea_b09_script as G  # noqa: E402

GROUPS = {
    "node-rendering": ["Color", "ColorRGBA", "Coordinate", "CoordinateDouble", "IndexedLineSet", "Normal", "PointSet"],
    "node-shape": ["Appearance", "Material", "Shape"],
    "node-lighting": ["DirectionalLight", "PointLight", "SpotLight"],
    "node-texturing": ["ImageTexture", "PixelTexture", "TextureTransform", "MovieTexture"],
    "node-texturing3d": ["TextureCoordinate3D", "TextureCoordinate4D"],
    "node-text": ["FontStyle", "Text"],
    "node-layout": ["LayoutGroup", "ScreenFontStyle", "ScreenGroup"],
    "node-environment": ["LocalFog", "TextureBackground"],
    "node-navigation": ["OrthoViewpoint"],
    "node-hanim": ["HAnimJoint", "HAnimSegment", "HAnimSite"],
    "node-sound": ["AudioClip", "Sound"],
    "node-dis": ["EspduTransform"],
}

COMPONENTS = {
    "node-texturing3d": "Texturing3D:2", "node-layout": "Layout:2", "node-environment": "EnvironmentalEffects:3",
    "node-hanim": "HAnim:1", "node-dis": "DIS:1",
}

# A sample value per field type: the field's constructor, so X_ITE's own classes make it.
PRE = r"""
var SAMPLE = {
  SFBool: function (v) { return !v; }, SFInt32: function () { return 3; }, SFFloat: function () { return 0.25; },
  SFDouble: function () { return 0.5; }, SFTime: function () { return 2; }, SFString: function () { return 'sample'; },
  SFVec2f: function () { return new SFVec2f (0.5, 0.25); }, SFVec2d: function () { return new SFVec2d (0.5, 0.25); },
  SFVec3f: function () { return new SFVec3f (0.5, 0.25, 0.125); }, SFVec3d: function () { return new SFVec3d (0.5, 0.25, 0.125); },
  SFVec4f: function () { return new SFVec4f (0.5, 0.25, 0.125, 1); }, SFVec4d: function () { return new SFVec4d (0.5, 0.25, 0.125, 1); },
  SFColor: function () { return new SFColor (0.5, 0.25, 0.125); }, SFColorRGBA: function () { return new SFColorRGBA (0.5, 0.25, 0.125, 0.75); },
  SFRotation: function () { return new SFRotation (0, 1, 0, 0.5); },
  SFMatrix3f: function () { return new SFMatrix3f (2, 0, 0, 0, 2, 0, 0, 0, 1); }, SFMatrix4f: function () { return new SFMatrix4f (2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 1, 2, 3, 1); },
  SFImage: function () { return new SFImage (1, 1, 3, new MFInt32 (0xFF8000)); },
  MFBool: function () { return new MFBool (true, false); }, MFInt32: function () { return new MFInt32 (1, 2, -1); },
  MFFloat: function () { return new MFFloat (0, 0.5, 1); }, MFDouble: function () { return new MFDouble (0, 0.5, 1); },
  MFTime: function () { return new MFTime (1, 2); }, MFString: function () { return new MFString ('a', 'b'); },
  MFVec2f: function () { return new MFVec2f (new SFVec2f (0, 0), new SFVec2f (1, 0.5)); },
  MFVec2d: function () { return new MFVec2d (new SFVec2d (0, 0), new SFVec2d (1, 0.5)); },
  MFVec3f: function () { return new MFVec3f (new SFVec3f (0, 0, 0), new SFVec3f (1, 0.5, 0.25)); },
  MFVec3d: function () { return new MFVec3d (new SFVec3d (0, 0, 0), new SFVec3d (1, 0.5, 0.25)); },
  MFVec4f: function () { return new MFVec4f (new SFVec4f (0, 0, 0, 1), new SFVec4f (1, 0.5, 0.25, 1)); },
  MFColor: function () { return new MFColor (new SFColor (1, 0, 0), new SFColor (0, 1, 0)); },
  MFColorRGBA: function () { return new MFColorRGBA (new SFColorRGBA (1, 0, 0, 1), new SFColorRGBA (0, 1, 0, 0.5)); },
  MFRotation: function () { return new MFRotation (new SFRotation (0, 1, 0, 1)); },
};
var EV = {};
function typeName (t) { for (var k in X3DConstants) if (X3DConstants [k] === t && /^(SF|MF)/ .test (k)) return k; return String (t); }
function defs (n) { return Array .from (n .getFieldDefinitions (), function (d) { return d .name + '/' + d .accessType + '/' + typeName (d .dataType) + '/' + String (d .value); }); }
"""


# Output fields left out of a group's callbacks (documented open, in NAME-outputs-open, which is generated but not baselined):
# Text's layout outputs, which X_ITE sends when it rebuilds the text, after its default font has loaded (twice for one change).
UNWATCHED = {"Text": ["origin", "textBounds", "lineBounds"]}


def world(name, types, unwatched=UNWATCHED):
    probes = []
    for t in types:
        probes.append((f"{t} defs", f"return defs (Browser .currentScene .createNode ('{t}'));"))
        probes.append((f"{t} created text", f"var n = Browser .currentScene .createNode ('{t}'); return [n .getNodeTypeName (), n .getType (), n .toVRMLString (), n .toXMLString (), n .toJSONString ()];"))
    watch, setv, read = [], [], []
    for t in types:
        watch.append(f"""  T ('{t} watch', function () {{ var n = Browser .currentScene .getNamedNode ('N_{t}'), e = EV ['{t}'] = []; var c = 0;
    Array .from (n .getFieldDefinitions ()) .forEach (function (d) {{ if (d .accessType === X3DConstants .initializeOnly || d .accessType === X3DConstants .inputOnly) return;
      if ({json.dumps(unwatched.get(t, []))} .indexOf (d .name) >= 0) return;
      ++ c; n .addFieldCallback ('w', d .name, function (v) {{ e .push (d .name + '=' + String (v)); }}); }}); return c; }});""")
        setv.append(f"""  T ('{t} set', function () {{ var n = Browser .currentScene .getNamedNode ('N_{t}'), s = [];
    Array .from (n .getFieldDefinitions ()) .forEach (function (d) {{ if (d .accessType !== X3DConstants .inputOnly && d .accessType !== X3DConstants .inputOutput) return;
      var f = SAMPLE [typeName (d .dataType)]; if (!f) return; try {{ n [d .name] = f (n [d .name]); s .push (d .name); }} catch (x) {{ s .push (d .name + ' ERR ' + x .name); }} }}); return s; }});""")
        read.append(f"""  T ('{t} events', function () {{ return EV ['{t}']; }});
  T ('{t} after', function () {{ var n = Browser .currentScene .getNamedNode ('N_{t}'); return n .toVRMLString (); }});""")
    after = ("var F_ = 0;\nfunction prepareEvents () {\n  ++ F_;\n  if (F_ === 2) {\n" + "\n".join(watch) + "\n  }\n  if (F_ === 3) {\n" + "\n".join(setv) +
             "\n  }\n  if (F_ === 6) {\n" + "\n".join(read) + "\n    flush_ ();\n  }\n}\n")
    keep = "  initializeOnly MFNode keep [ " + " ".join(f"DEF N_{t} {t} {{ }}" for t in types) + " ]\n"
    G.register(name, probes, fields=keep, pre=PRE, after=after)


for name, types in GROUPS.items():
    world(name, types)

world("node-text-outputs-open", ["Text"], unwatched={})

if __name__ == "__main__":
    head = G.HEAD
    for name in list(GROUPS) + ["node-text-outputs-open"]:
        G.HEAD = head + (f"COMPONENT {COMPONENTS[name]}\n" if name in COMPONENTS else "")
        sys.argv = [sys.argv[0], name]
        G.main()
    G.HEAD = head
