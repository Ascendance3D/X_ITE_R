#!/usr/bin/env python3
"""REA batch 12 Billboard lane: bboxDisplay of Billboard (Billboard.getBBox: the children's box times Billboard.matrix, the rotation
of the last DISPLAY traversal) and Billboards under shadows, as stepped worlds + key scripts (oracle/synthetic-rea-b12-billboard).
512x512, dt = 1/30, same idiom as gen_rea_b11_bbox.py.

  bbb-static   axisOfRotation Y / 0 0 0 / X, a parent Transform with rotation and scale, empty children, explicit bboxSize, nested
               Billboards, a Billboard in a Group with bboxDisplay
  bbb-camera   the camera moves (a viewpoint animated by key-driven set_bind / position changes and an Examine drag), so the
               rotation changes every frame
  bbb-dynamic  run-time changes: axisOfRotation, children added to an empty Billboard, bboxDisplay toggled, parent rotation
  bbb-shadow   a Billboard casting a shadow (X_ITE draws the SHADOW pass with Billboard.matrix, not a light-space rotation)

  python3 -I -B oracle/tools/gen_rea_b12_billboard.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b12-billboard"))
os.makedirs(root, exist_ok=True)
HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
BG = "Background { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
SOLID = lambda c: f"Appearance {{ material Material {{ diffuseColor {c} }} }}"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, frames, events, capture):
    d = {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": sorted(events, key=lambda e: e["frame"])}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)

def keys(letters, step=8, start=6):
    return [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]

def box(size, color, tr=None):
    s = f"Shape {{ appearance {SOLID(color)} geometry Box {{ size {size} }} }}"
    return f"Transform {{ translation {tr} children {s} }}" if tr else s

def script_node(names, body):
    refs = "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in names)
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + refs
            + "  url \"javascript:\n" + body + "\n\"\n}\nROUTE K.keyPress TO R.on_key\n")

VP = "DEF V1 Viewpoint { position 2 1.5 14 orientation 0 1 0 0.12 }\n"
PANEL = lambda c, tr="0.6 0 0": box("1.6 0.8 0.2", c, tr)

# ---- static ------------------------------------------------------------------------------------------------------------
st = HEAD + VP + BG
st += f"Transform {{ translation -4 3 0 children Billboard {{ bboxDisplay TRUE children [ {PANEL('0.3 0.5 0.9')} ] }} }}\n"
st += f"Transform {{ translation 0 3 0 children Billboard {{ bboxDisplay TRUE axisOfRotation 0 0 0 children [ {PANEL('0.9 0.6 0.2')} ] }} }}\n"
st += f"Transform {{ translation 4 3 0 children Billboard {{ bboxDisplay TRUE axisOfRotation 1 0 0 children [ {PANEL('0.4 0.9 0.4', '0 0.5 0.3')} ] }} }}\n"
st += (f"Transform {{ translation -4 0 0 rotation 0 0 1 0.4 scale 1.5 0.8 1 children Billboard {{ bboxDisplay TRUE children [ {PANEL('0.9 0.3 0.3')} ] }} }}\n")
st += "Transform { translation 0 0 0 children Billboard { bboxDisplay TRUE } }\n"
st += f"Transform {{ translation 4 0 0 children Billboard {{ bboxDisplay TRUE bboxSize 2 1 0.5 bboxCenter 0.3 0.2 0 children [ {PANEL('0.7 0.7 0.2')} ] }} }}\n"
st += (f"Transform {{ translation -3 -3 0 children Billboard {{ bboxDisplay TRUE children [ {PANEL('0.5 0.3 0.8')} "
       f"Transform {{ translation 1.5 0 0 children Billboard {{ bboxDisplay TRUE axisOfRotation 0 0 0 children [ {PANEL('0.2 0.8 0.8', '0 0.6 0')} ] }} }} ] }} }}\n")
st += f"Transform {{ translation 3 -3 0 children Group {{ bboxDisplay TRUE children [ Billboard {{ children [ {PANEL('0.8 0.5 0.5')} ] }} ] }} }}\n"
write("bbb-static", st)
script("bbb-static", 8, [], [2, 6])

# ---- camera: the viewpoint moves and turns every few frames; an Examine drag turns the user's view --------------------------
cam = HEAD + VP + BG
cam += f"Transform {{ translation -2 1 0 children Billboard {{ bboxDisplay TRUE children [ {PANEL('0.3 0.5 0.9')} ] }} }}\n"
cam += f"Transform {{ translation 2 1 0 children Billboard {{ bboxDisplay TRUE axisOfRotation 0 0 0 children [ {PANEL('0.9 0.6 0.2')} ] }} }}\n"
cam += (f"Transform {{ translation 0 -2 0 rotation 1 0 0 0.3 children Billboard {{ bboxDisplay TRUE axisOfRotation 0 0 1 children [ {PANEL('0.4 0.9 0.4')} ] }} }}\n")
cam += script_node(["V1"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') { v1 .position = new SFVec3f (-6, 2, 11); v1 .orientation = new SFRotation (0, 1, 0, -0.5); }
   if (k == 'b') { v1 .position = new SFVec3f (5, -3, 9); v1 .orientation = new SFRotation (1, 1, 0, 0.45); }
   if (k == 'c') { v1 .position = new SFVec3f (0, 8, 6); v1 .orientation = new SFRotation (1, 0, 0, -0.9); }
}""")
write("bbb-camera", cam)
ev = keys("abc", step=10)
ev += [{"frame": 40, "type": "mousedown", "button": 0, "x": 256, "y": 256}] + [{"frame": 41 + i, "type": "mousemove", "x": 256 + 12 * (i + 1), "y": 256 + 5 * (i + 1)} for i in range(6)]
ev += [{"frame": 47, "type": "mouseup", "button": 0, "x": 328, "y": 286}]
script("bbb-camera", 56, ev, [4, 9, 19, 29, 42, 44, 46, 50, 54])

# ---- dynamic: run-time changes ------------------------------------------------------------------------------------------
dy = HEAD + VP + BG
dy += f"DEF P1 Transform {{ translation -3 1 0 children DEF B1 Billboard {{ bboxDisplay TRUE children [ {PANEL('0.3 0.5 0.9')} ] }} }}\n"
dy += "Transform { translation 2 1 0 children DEF B2 Billboard { bboxDisplay TRUE } }\n"
dy += f"Transform {{ translation 0 -2 0 children DEF B3 Billboard {{ children [ {PANEL('0.9 0.6 0.2')} ] }} }}\n"
dy += script_node(["B1", "B2", "B3", "P1"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') b1 .axisOfRotation = new SFVec3f (0, 0, 0);
   if (k == 'b') b2 .children = new MFNode (b3 .children [0]);
   if (k == 'c') b3 .bboxDisplay = true;
   if (k == 'd') p1 .rotation = new SFRotation (0, 0, 1, 0.6);
   if (k == 'e') b1 .axisOfRotation = new SFVec3f (1, 1, 0);
   if (k == 'f') b2 .bboxDisplay = false;
}""")
write("bbb-dynamic", dy)
script("bbb-dynamic", 60, keys("abcdef"), [4] + [6 + 8 * i + 3 for i in range(6)])

# ---- shadow: a Billboard casts a shadow onto a floor ---------------------------------------------------------------------
sh = HEAD + "DEF V1 Viewpoint { position 0 4 12 orientation 1 0 0 -0.3 }\n" + BG
sh += "DirectionalLight { direction -0.4 -1 -0.3 shadows TRUE shadowIntensity 0.8 }\n"
sh += "NavigationInfo { headlight FALSE }\n"
sh += f"Transform {{ translation 0 1.5 0 children Billboard {{ bboxDisplay TRUE children [ {box('3 2 0.1', '0.9 0.6 0.2', '1 0 0')} ] }} }}\n"
sh += f"Transform {{ translation -3 1 1 children Billboard {{ axisOfRotation 0 0 0 children [ {box('1.5 1.5 0.1', '0.3 0.5 0.9', '0.5 0.3 0')} ] }} }}\n"
sh += f"Transform {{ translation 0 -1 0 children {box('14 0.2 10', '0.8 0.8 0.8')} }}\n"
write("bbb-shadow", sh)
script("bbb-shadow", 8, [], [2, 6])
