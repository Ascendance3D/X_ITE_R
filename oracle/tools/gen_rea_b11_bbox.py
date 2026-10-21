#!/usr/bin/env python3
"""REA batch 11 lane b11-1-bbox: bboxDisplay of Group, Transform, ScreenGroup (and the other X3DBoundedObjects the grouping code handles)
as stepped worlds + key scripts (oracle/synthetic-rea-b11-bbox). 512x512, dt = 1/30. Same idiom as gen_rea_b10_screen.py.

  python3 -I oracle/tools/gen_rea_b11_bbox.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b11-bbox"))
os.makedirs(root, exist_ok=True)
cases = []
HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT Layout:2\n"
BG = "Background { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
EMIT = lambda c: f"Appearance {{ material Material {{ emissiveColor {c} diffuseColor 0 0 0 }} }}"
SOLID = lambda c: f"Appearance {{ material Material {{ diffuseColor {c} }} }}"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, frames, events, capture):
    d = {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": sorted(events, key=lambda e: e["frame"])}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)
    cases.append(name)

def keyscript(name, letters, step=8, start=6, tail=6, after=3):
    ev = [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]
    script(name, start + step * len(letters) + tail, ev, [start - 2] + [start + step * i + after for i in range(len(letters))])

def box(size, color, tr=None):
    s = f"Shape {{ appearance {SOLID(color)} geometry Box {{ size {size} }} }}"
    return f"Transform {{ translation {tr} children {s} }}" if tr else s

def sphere(r, color):
    return f"Shape {{ appearance {SOLID(color)} geometry Sphere {{ radius {r} }} }}"

def rect(w, h, color, defs=""):
    pts = f"-{w} -{h} 0, {w} -{h} 0, {w} {h} 0, -{w} {h} 0"
    co = f"DEF {defs} Coordinate" if defs else "Coordinate"
    return f"Shape {{ appearance {EMIT(color)} geometry IndexedFaceSet {{ coord {co} {{ point [ {pts} ] }} coordIndex [ 0 1 2 3 -1 ] }} }}"

def script_node(fields, body):
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + fields
            + "\n  url \"javascript:\n" + body + "\n\"\n}\nROUTE K.keyPress TO R.on_key\n")

def refs(names):
    return "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in names)

VP = "DEF V1 Viewpoint { position 0 0 14 }\n"

# ---- static: normal geometry, transformed children, nested groups, empty groups, explicit bbox ---------------------------
st = HEAD + VP + BG
st += "Transform { translation -4 3 0 children Group { bboxDisplay TRUE children [ " + box("1.5 1 1", "0.3 0.5 0.9") + " ] } }\n"
st += ("Transform { translation 0 3 0 children Transform { bboxDisplay TRUE translation 0.5 0 0 rotation 0 0 1 0.5 scale 1 2 1 children [ "
       + box("1 1 1", "0.9 0.6 0.2") + " ] } }\n")
st += ("Transform { translation 4 3 0 children Group { bboxDisplay TRUE children [ Transform { translation -0.6 0 0 rotation 0 1 0 0.7 children "
       + sphere(0.6, "0.4 0.9 0.4") + " } Transform { translation 0.9 0.5 0.3 children Group { bboxDisplay TRUE children [ " + box("0.5 0.5 0.5", "0.9 0.3 0.3") + " ] } } ] } }\n")
st += "Transform { translation -4 0 0 children Group { bboxDisplay TRUE } }\n"
st += "Transform { translation -4 0 0 children Transform { bboxDisplay TRUE translation 0 -0.7 0 } }\n"
st += ("Transform { translation 0 0 0 children Group { bboxDisplay TRUE bboxSize 3 1 2 bboxCenter 0.5 0.5 0 children [ " + box("1 1 1", "0.7 0.7 0.2") + " ] } }\n")
st += ("Transform { translation 4 0 0 children Transform { bboxDisplay TRUE bboxSize 2 2 1 bboxCenter 0 0 0.5 rotation 1 0 0 0.6 scale 1.5 1 1 children [ "
       + box("0.5 0.5 0.5", "0.2 0.7 0.9") + " ] } }\n")
st += "Transform { translation -4 -3 0 children Group { bboxDisplay TRUE visible FALSE children [ " + box("1 1 1", "0.9 0.9 0.9") + " ] } }\n"
st += "Transform { translation 0 -3 0 children Group { bboxDisplay FALSE children [ " + box("1 1 1", "0.5 0.5 0.5") + " ] } }\n"
st += "Transform { translation 4 -3 0 children Shape { bboxDisplay TRUE appearance " + SOLID("0.9 0.4 0.7") + " geometry Cone { height 1.5 bottomRadius 0.6 } } }\n"
write("bb-static", st)
script("bb-static", 12, [], [8])

# ---- the other bounded objects (Switch / Collision / Anchor / LOD / Shape castShadow FALSE) -------------------------------------
ot = HEAD + VP + BG
ot += ("Transform { translation -4 2 0 children Switch { bboxDisplay TRUE whichChoice 1 children [ " + box("1 1 1", "0.9 0.2 0.2")
       + " Transform { translation 0.3 0 0 children " + sphere(0.5, "0.2 0.9 0.2") + " } ] } }\n")
ot += "Transform { translation 0 2 0 children Collision { bboxDisplay TRUE children [ " + box("1 1.4 1", "0.3 0.3 0.9", "0.2 0 0") + " ] } }\n"
ot += "Transform { translation 4 2 0 children Anchor { bboxDisplay TRUE url \"x.wrl\" children [ " + box("1 1 1", "0.9 0.9 0.2") + " ] } }\n"
ot += ("Transform { translation -4 -2 0 children LOD { bboxDisplay TRUE range [ 100 ] children [ " + box("1.2 0.6 0.6", "0.5 0.9 0.9") + " "
       + box("2 2 2", "0.2 0.2 0.2") + " ] } }\n")
ot += ("Transform { translation 0 -2 0 children Group { bboxDisplay TRUE children [ Shape { castShadow FALSE appearance " + SOLID("0.9 0.5 0.1")
       + " geometry Box { size 1 1 2 } } ] } }\n")
ot += "Transform { translation 4 -2 0 children Group { bboxDisplay TRUE children [ Transform { visible FALSE translation 2 0 0 children " + box("1 1 1", "0.9 0.9 0.9") + " } " + box("0.5 0.5 0.5", "0.4 0.8 0.4") + " ] } }\n"
write("bb-others", ot)
script("bb-others", 12, [], [8])

# ---- ScreenGroup: default / explicit bbox, nested, under a rotated parent, perspective and ortho, ContentScale ----------------
sc = HEAD + VP + "DEF O1 OrthoViewpoint { position 0 0 10 fieldOfView [ -7 -7 7 7 ] }\n" + BG
sc += "Transform { translation -4 3 0 children ScreenGroup { bboxDisplay TRUE children [ " + rect(60, 40, "1 0.4 0.1") + " ] } }\n"
sc += "Transform { translation 0 3 0 children ScreenGroup { bboxDisplay TRUE bboxSize 80 40 1 bboxCenter 10 0 0 children [ " + rect(30, 30, "0.2 0.9 0.4") + " ] } }\n"
sc += ("Transform { translation 4 3 -3 rotation 0 1 0 0.6 children ScreenGroup { bboxDisplay TRUE children [ " + rect(50, 30, "0.9 0.9 0.2")
       + " ScreenGroup { bboxDisplay TRUE children [ Transform { translation 20 10 0 children " + rect(12, 12, "0.9 0.2 0.9") + " } ] } ] } }\n")
sc += "Transform { translation -4 -1 0 children ScreenGroup { bboxDisplay TRUE } }\n"
sc += ("Transform { translation 0 -1 0 children ScreenGroup { children [ " + rect(40, 40, "0.3 0.6 1") + " Group { bboxDisplay TRUE children [ "
       + "Transform { translation 25 0 0 children " + rect(10, 10, "1 1 1") + " } ] } ] } }\n")
sc += "Transform { translation 4 -1 0 children Group { bboxDisplay TRUE children [ ScreenGroup { children [ " + rect(40, 20, "0.8 0.5 0.8") + " ] } ] } }\n"
sc += "Transform { translation -4 -4 0 children ScreenGroup { bboxDisplay TRUE children [ Shape { appearance " + EMIT("1 1 1") + " geometry Text { string [ \"Box\" \"Text\" ] fontStyle ScreenFontStyle { pointSize 20 } solid FALSE } } ] } }\n"
sc += "Transform { translation 4 -4 0 children Transform { bboxDisplay TRUE children ScreenGroup { children [ " + rect(30, 50, "0.9 0.6 0.3") + " ] } } }\n"
sc += script_node(refs(["O1", "V1"]), """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') o1 .set_bind = true;
   if (k == 'b') Browser .setBrowserOption ('ContentScale', 2);
   if (k == 'c') v1 .set_bind = true;
   if (k == 'd') Browser .setBrowserOption ('ContentScale', 1);
}""")
write("bb-screen", sc)
keyscript("bb-screen", "abcd")

# ---- run-time changes: toggles, child moves, bboxSize / bboxCenter, add / remove children, ContentScale (X3DBBoxNode epsilon) --------
dy = HEAD + VP + BG
dy += "DEF T1 Transform { translation -3 2 0 children DEF G1 Group { children [ " + box("1.5 1 1", "0.3 0.5 0.9") + " DEF M1 Transform { translation 1 0 0 children " + sphere(0.4, "0.9 0.5 0.2") + " } ] } }\n"
dy += "DEF T2 Transform { translation 3 2 0 children DEF G2 Transform { bboxDisplay TRUE children [ " + box("1 1 1", "0.4 0.8 0.4") + " ] } }\n"
dy += "DEF T3 Transform { translation -3 -2 0 children DEF G3 Group { bboxDisplay TRUE children [ ] } }\n"
dy += "DEF T4 Transform { translation 3 -2 0 children DEF G4 ScreenGroup { bboxDisplay TRUE children [ " + rect(40, 30, "0.8 0.3 0.8") + " ] } }\n"
dy += script_node(refs(["T1", "G1", "M1", "G2", "G3", "G4"]), """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') g1 .bboxDisplay = true;
   if (k == 'b') m1 .translation = new SFVec3f (2, 1, 0);
   if (k == 'c') t1 .scale = new SFVec3f (1, 2, 1);
   if (k == 'd') g2 .rotation = new SFRotation (0, 0, 1, 0.7);
   if (k == 'e') g2 .scale = new SFVec3f (2, 1, 1);
   if (k == 'f') g3 .children = new MFNode (m1);
   if (k == 'g') g1 .bboxDisplay = false;
   if (k == 'h') Browser .setBrowserOption ('ContentScale', 2);
   if (k == 'i') Browser .setBrowserOption ('ContentScale', 1);
}""")
write("bb-dynamic", dy)
keyscript("bb-dynamic", "abcdefghi")

open(os.path.join(root, "cases.txt"), "w").write("".join(f"{n} oracle/synthetic-rea-b11-bbox/{n}.wrl oracle/synthetic-rea-b11-bbox/{n}.json\n" for n in cases))
print(cases)
