#!/usr/bin/env python3
"""REA batch 11 lane b11-3-ortho: OrthoViewpoint viewAll / LookAt as stepped worlds + key scripts (oracle/synthetic-rea-b11-ortho).
512x512, dt = 1/30, KeySensor driven like gen_rea_b10_secondary.py. Cases are added to oracle/rea-batch10-baseline/cases.txt by
`rea_b10.py stepped record`.

  python3 -I oracle/tools/gen_rea_b11_ortho.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b11-ortho"))
os.makedirs(root, exist_ok=True)
HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
BG = "Background { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
SOLID = lambda c: f"Appearance {{ material Material {{ diffuseColor {c} }} }}"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, frames, events, capture):
    d = {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": sorted(events, key=lambda e: e["frame"]), "buildEachFrame": True}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)

def keyscript(name, letters, extra=(), step=8, start=6, tail=10, after=3):
    ev = [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]
    capture = [start - 2] + [start + step * i + after for i in range(len(letters))]
    script(name, start + step * len(letters) + tail, ev + list(extra), capture)

def script_node(names, body):
    refs = "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in names)
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + refs
            + "  url \"javascript:\n" + body + "\n\"\n}\nROUTE K.keyPress TO R.on_key\n")

SCENE = ("DEF SW Switch { whichChoice 0 children [ DEF GS Transform { scale 1.2 1 1 children [\n"
         f"  Transform {{ translation -1.5 0 0 children Shape {{ appearance {SOLID('0.3 0.5 0.9')} geometry Box {{ size 1.5 1.5 1.5 }} }} }}\n"
         f"  Transform {{ translation 1.5 0 0 children Shape {{ appearance {SOLID('0.9 0.6 0.2')} geometry Sphere {{ radius 0.9 }} }} }}\n"
         f"  DEF FAR Transform {{ translation 0 2.5 -2 children Shape {{ appearance {SOLID('0.4 0.9 0.4')} geometry Box {{ size 1 1 1 }} }} }}\n"
         "] } ] }\n")

# Scaled, translated, rotated parent Transform around the first (bound) OrthoViewpoint; a second OrthoViewpoint with
# retainUserOffsets, a perspective Viewpoint; non-square canvases; an empty scene.
w = HEAD + BG
w += "DEF TV Transform { translation 1 0 0 rotation 0 1 0 0.2 scale 2 1 1.5 children [ DEF O1 OrthoViewpoint { position 0 0 10 fieldOfView [ -2 -1 2 1 ] viewAll TRUE } ] }\n"
w += "DEF O2 OrthoViewpoint { position 3 2 10 orientation 0 1 0 0.3 fieldOfView [ -3 -3 3 3 ] retainUserOffsets TRUE }\n"
w += "DEF P Viewpoint { position 0 0 12 }\n"
w += SCENE
w += script_node(["O1", "O2", "P", "GS", "SW", "FAR"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') o1 .viewAll = true;
   if (k == 'b') far .translation = new SFVec3f (-9, 4, -2);
   if (k == 'c') o1 .viewAll = true;
   if (k == 'd') o1 .fieldOfView = new MFFloat (-4, -2, 4, 2);
   if (k == 'e') o1 .viewAll = true;
   if (k == 'f') o1 .viewAll = false;
   if (k == 'g') o2 .set_bind = true;
   if (k == 'h') o2 .viewAll = true;
   if (k == 'i') p .viewAll = true;
   if (k == 'j') p .set_bind = true;
   if (k == 'k') o1 .set_bind = true;
   if (k == 'l') gs .scale = new SFVec3f (0.4, 0.4, 0.4);
   if (k == 'm') o1 .viewAll = true;
   if (k == 'n') sw .whichChoice = -1;
   if (k == 'o') o1 .viewAll = true;
}""")
write("ov-view2", w)
keyscript("ov-view2", "abcdefghijklmno", extra=[
    {"frame": 18, "type": "resize", "width": 640, "height": 360}, {"frame": 58, "type": "resize", "width": 300, "height": 500},
    {"frame": 140, "type": "resize", "width": 512, "height": 512}])

# LookAt on an OrthoViewpoint: LOOKAT viewer clicks at several points, with the bound OrthoViewpoint at different offsets.
l = HEAD + BG
l += "DEF O1 OrthoViewpoint { position 0 0 10 fieldOfView [ -4 -4 4 4 ] }\n"
l += SCENE
l += script_node(["O1"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') o1 .viewAll = true;
}""")
write("ov-lookat", l)
look = [{"frame": 12, "type": "viewer", "name": "LOOKAT"}]
for i, (x, y) in enumerate([(300, 256), (200, 256), (256, 120)]):
    f = 14 + 40 * i
    look += [{"frame": f, "type": "mousedown", "button": 0, "x": x, "y": y}, {"frame": f + 1, "type": "mouseup", "button": 0, "x": x, "y": y}]
look.append({"frame": 134, "type": "key", "key": "a"})
script("ov-lookat", 150, look, [10, 52, 92, 132, 144])
