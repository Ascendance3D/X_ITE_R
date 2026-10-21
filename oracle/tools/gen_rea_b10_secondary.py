#!/usr/bin/env python3
"""REA batch 10 secondary GPU work: stepped worlds for LayoutGroup (run-time layout / viewport / children changes, canvas resize),
OrthoViewpoint (viewAll, lookAt) and LocalFog on a volume draw (oracle/synthetic-rea-b10-secondary). 512x512, dt = 1/30, KeySensor
driven like gen_rea_b10_screen.py. Cases are listed in oracle/rea-batch10-baseline/cases.txt by `rea_b10.py stepped record`.

  python3 -I oracle/tools/gen_rea_b10_secondary.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b10-secondary"))
os.makedirs(root, exist_ok=True)
HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT Layout:2\n"
BG = "Background { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
BASE = ("Transform { translation -1.5 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.3 0.5 0.9 } } geometry Box { size 1.5 1.5 1.5 } } }\n"
        "Transform { translation 1.5 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.6 0.2 } } geometry Sphere { radius 0.9 } } }\n")
EMIT = lambda c: f"Appearance {{ material Material {{ emissiveColor {c} diffuseColor 0 0 0 }} }}"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, frames, events, capture, settle=False, build_each=False):
    d = {"settle": settle, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": sorted(events, key=lambda e: e["frame"])}
    if build_each:
        d["buildEachFrame"] = True
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)

def keys(letters, step=8, start=6):
    return [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]

def keyscript(name, letters, extra=(), step=8, start=6, tail=6, after=3, **kw):
    capture = [start - 2] + [start + step * i + after for i in range(len(letters))]
    script(name, start + step * len(letters) + tail, keys(letters, step, start) + list(extra), capture, **kw)

def rect(w, h, color):
    return (f"Shape {{ appearance {EMIT(color)} geometry IndexedFaceSet {{ coord Coordinate {{ point [ {-w/2} {-h/2} 0, {w/2} {-h/2} 0, {w/2} {h/2} 0, {-w/2} {h/2} 0 ] }} "
            "coordIndex [ 0 1 2 3 -1 ] } }")

def script_node(names, body):
    refs = "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in names)
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + refs
            + "  url \"javascript:\n" + body + "\n\"\n}\nROUTE K.keyPress TO R.on_key\n")

LP = 'sizeUnits [ "PIXEL" "PIXEL" ] scaleMode [ "PIXEL" "PIXEL" ]'

# ---- LayoutGroup: layout fields, viewport, children, canvas size ---------------------------------------
lg = HEAD + "Viewpoint { position 0 0 8 }\n" + BG + BASE
lg += f"DEF LG1 LayoutGroup {{ layout DEF LAY1 Layout {{ align [ \"LEFT\" \"BOTTOM\" ] offsetUnits [ \"PIXEL\" \"PIXEL\" ] offset [ 10 10 ] size [ 100 50 ] {LP} }} children [ " + rect(100, 50, "1 0.4 0.1") + " ] }\n"
lg += f"DEF LG2 LayoutGroup {{ layout DEF LAY2 Layout {{ align [ \"CENTER\" \"CENTER\" ] size [ 120 60 ] {LP} }} viewport DEF VP Viewport {{ clipBoundary [ 0.4 0.6 0.4 0.6 ] }} children [ " + rect(60, 30, "0.9 0.9 0.2") + " " + rect(20, 20, "0.1 0.1 0.1") + " ] }\n"
lg += f"DEF LG3 LayoutGroup {{ layout Layout {{ align [ \"RIGHT\" \"TOP\" ] offset [ -10 -10 ] offsetUnits [ \"PIXEL\" \"PIXEL\" ] size [ 80 80 ] {LP} }} children [ DEF CH1 Transform {{ children " + rect(80, 80, "0.2 0.9 0.4") + " } ] }\n"
lg += script_node(["LAY1", "LAY2", "VP", "LG2", "LG3", "CH1"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') lay1 .align = new MFString ('RIGHT', 'BOTTOM');
   if (k == 'b') lay1 .offset = new MFFloat (10, 40);
   if (k == 'c') lay1 .size = new MFFloat (160, 80);
   if (k == 'd') lay1 .scaleMode = new MFString ('STRETCH', 'STRETCH');
   if (k == 'e') lg2 .addChildren = Browser .createVrmlFromString ('Shape { appearance Appearance { material Material { emissiveColor 0.9 0.2 0.9 diffuseColor 0 0 0 } } geometry IndexedFaceSet { coord Coordinate { point [ -20 -20 0, 20 -20 0, 20 20 0, -20 20 0 ] } coordIndex [ 0 1 2 3 -1 ] } }');
   if (k == 'f') vp .clipBoundary = new MFFloat (0.2, 0.8, 0.2, 0.8);
   if (k == 'g') ch1 .translation = new SFVec3f (-40, -20, 0);
   if (k == 'h') lay2 .align = new MFString ('CENTER', 'TOP');
   if (k == 'i') lg3 .visible = false;
}""")
write("lg-runtime", lg)
keyscript("lg-runtime", "abcdefghi", extra=[{"frame": 84, "type": "resize", "width": 640, "height": 360}, {"frame": 94, "type": "resize", "width": 300, "height": 500}], tail=30, settle=True)

# ---- OrthoViewpoint: viewAll and lookAt ----------------------------------------------------------------
ov = HEAD + "DEF O1 OrthoViewpoint { position 0 0 10 fieldOfView [ -2 -2 2 2 ] }\nDEF O2 OrthoViewpoint { position 3 2 10 orientation 0 1 0 0.3 fieldOfView [ -3 -3 3 3 ] }\n" + BG
ov += BASE + "Transform { translation 0 2.5 -2 children Shape { appearance Appearance { material Material { diffuseColor 0.4 0.9 0.4 } } geometry Box { size 1 1 1 } } }\n"
ov += "DEF FAR Transform { translation -4 -2 -3 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.4 0.4 } } geometry Sphere { radius 0.6 } } }\n"
ov += script_node(["O1", "O2", "FAR"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') o1 .viewAll = true;
   if (k == 'b') far .translation = new SFVec3f (-8, -2, -3);
   if (k == 'c') o1 .viewAll = true;
   if (k == 'd') o2 .set_bind = true;
   if (k == 'e') o2 .viewAll = true;
   if (k == 'f') o2 .fieldOfView = new MFFloat (-1, -1, 1, 1);
   if (k == 'g') o1 .set_bind = true;
}""")
write("ov-view", ov)
look = [{"frame": 70, "type": "viewer", "name": "LOOKAT"}, {"frame": 72, "type": "mousedown", "button": 0, "x": 330, "y": 256},
        {"frame": 73, "type": "mouseup", "button": 0, "x": 330, "y": 256}]
keyscript("ov-view", "abcdefg", extra=look, tail=70, after=4, build_each=True)

# ---- LocalFog on a volume draw -------------------------------------------------------------------------
vals = []
for k in range(4):
    for j in range(4):
        for i in range(4):
            a = 255 if (i + j + k) % 2 == 0 else 90
            vals.append(f"0x{(60 + 60 * i) & 255:02X}{(60 + 60 * j) & 255:02X}{(60 + 60 * k) & 255:02X}{a:02X}")
vol = "PixelTexture3D { image [ 4 4 4 4 " + " ".join(vals) + " ] }"
fv = HEAD + "COMPONENT VolumeRendering:2\nViewpoint { position 0 0 8 }\n" + BG
fv += "DEF LF LocalFog { color 0.8 0.4 0.3 visibilityRange 9 children [\n"
fv += "  Transform { translation -1.5 0 -1 children VolumeData { dimensions 2 2 2 voxels " + vol + " } }\n"
fv += "  Transform { translation 1.5 0 -5 children VolumeData { dimensions 2 2 2 voxels " + vol + " } }\n"
fv += "  Transform { translation 0 -2 -2 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.6 0.2 } } geometry Sphere { radius 0.8 } } }\n] }\n"
fv += "Transform { translation 0 2.2 -3 children VolumeData { dimensions 1.6 1.6 1.6 voxels " + vol + " } }\n"
fv += script_node(["LF"], """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') lf .color = new SFColor (0.2, 0.5, 0.9);
   if (k == 'b') lf .visibilityRange = 5;
   if (k == 'c') lf .fogType = 'EXPONENTIAL';
   if (k == 'd') lf .enabled = false;
   if (k == 'e') lf .enabled = true;
   if (k == 'f') lf .visibilityRange = 0;
}""")
write("lf-volume", fv)
keyscript("lf-volume", "abcdef", settle=True)
print(sorted(f for f in os.listdir(root) if f.endswith(".wrl")))
