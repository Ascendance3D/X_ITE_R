#!/usr/bin/env python3
"""REA batch 10 lane Text / ScreenGroup / ScreenFontStyle: stepped (run-time) worlds and event scripts
(oracle/synthetic-rea-b10-stepped). A KeySensor drives a Script that changes the live scene (parent transforms,
child geometry, viewpoints, bboxDisplay, ContentScale, FontStyle / ScreenFontStyle fields). 512x512, dt = 1/30.
cases.txt lists `name world script`. Same idiom as gen_rea_b09_stepped.py.

  python3 -I oracle/tools/gen_rea_b10_screen.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b10-stepped"))
os.makedirs(root, exist_ok=True)
cases = []
HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT Layout:2\n"
BG = "Background { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
BASE = ("Transform { translation -1.5 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.3 0.5 0.9 } } geometry Box { size 1.5 1.5 1.5 } } }\n"
        "Transform { translation 1.5 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.6 0.2 } } geometry Sphere { radius 0.9 } } }\n")
EMIT = lambda c: f"Appearance {{ material Material {{ emissiveColor {c} diffuseColor 0 0 0 }} }}"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, frames, events, capture, settle=False):
    ev = sorted(events, key=lambda e: e["frame"])
    d = {"settle": settle, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": ev}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)
    cases.append(name)

def keyscript(name, letters, step=8, start=6, tail=6, after=3, settle=False):
    """Press `letters` every `step` frames; capture one frame before the first and `after` frames after each press (`settle` makes a font file a
    FontStyle change fetches arrive between two frames in X_ITE and Rust alike; see x_ite-anim.mjs)."""
    ev = [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]
    script(name, start + step * len(letters) + tail, ev, [start - 2] + [start + step * i + after for i in range(len(letters))], settle)

def rect(w, h, color, defs=""):
    pts = f"-{w} -{h} 0, {w} -{h} 0, {w} {h} 0, -{w} {h} 0"
    co = f"DEF {defs} Coordinate" if defs else "Coordinate"
    return f"Shape {{ appearance {EMIT(color)} geometry IndexedFaceSet {{ coord {co} {{ point [ {pts} ] }} coordIndex [ 0 1 2 3 -1 ] }} }}"

def script_node(fields, body):
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + fields
            + "\n  url \"javascript:\n" + body + "\n\"\n}\nROUTE K.keyPress TO R.on_key\n")

def refs(names):
    return "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in names)

# ---- ScreenGroup: parent / child changes, nesting, bounding box, viewpoints, ContentScale -----------
sg = HEAD + "DEF V1 Viewpoint { position 0 0 8 description \"one\" }\nDEF V2 Viewpoint { position 3 1 6 orientation 0 1 0 0.3 fieldOfView 0.7 }\n"
sg += "DEF O1 OrthoViewpoint { position 0 0 10 fieldOfView [ -5 -4 5 4 ] }\n" + BG + BASE
sg += "DEF T1 Transform { translation -2.5 1.5 0 children DEF SG1 ScreenGroup { children [ " + rect(60, 40, "1 0.4 0.1", "C1") + " ] } }\n"
sg += ("DEF T2 Transform { translation 2.5 1.5 0 children DEF SG2 ScreenGroup { children [ " + rect(50, 30, "0.2 0.9 0.4")
       + " DEF SG3 ScreenGroup { children [ Transform { translation 20 10 0 children " + rect(12, 12, "0.9 0.2 0.9") + " } ] } ] } }\n")
sg += ("DEF T3 Transform { translation -2.5 -2 0 children ScreenGroup { children [ Shape { appearance " + EMIT("1 1 1")
       + " geometry DEF TX Text { string [ \"Screen\" ] fontStyle DEF SFS ScreenFontStyle { pointSize 24 } solid FALSE } } ] } }\n")
sg += "DEF T4 Transform { translation 2.5 -2 0 children DEF SG4 ScreenGroup { children [ " + rect(30, 20, "0.9 0.9 0.2") + " ] } }\n"
sg += ("DEF T5 Transform { translation 0 2.8 -3 children ScreenGroup { children [ "
       + rect(30, 20, "0.5 0.5 0.5") + " ] } }\n")
sg += script_node(refs(["T1", "SG1", "C1", "TX", "SFS", "SG2", "SG3", "SG4", "V2", "O1", "V1"]),
"""function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') t1 .translation = new SFVec3f (-1, 1.5, 0);
   if (k == 'b') t1 .rotation = new SFRotation (0, 0, 1, 0.5);
   if (k == 'c') t1 .scale = new SFVec3f (2, 1, 1);
   if (k == 'd') c1 .point = new MFVec3f (new SFVec3f (-30, -20, 0), new SFVec3f (30, -20, 0), new SFVec3f (30, 20, 0), new SFVec3f (0, 40, 0));
   if (k == 'e') tx .string = new MFString ('Changed', 'text');
   if (k == 'f') sfs .pointSize = 40;
   if (k == 'g') sg4 .bboxDisplay = true;
   if (k == 'h') sg2 .bboxDisplay = true;
   if (k == 'i') v2 .set_bind = true;
   if (k == 'j') o1 .set_bind = true;
   if (k == 'k') v1 .set_bind = true;
   if (k == 'l') sg3 .visible = false;
   if (k == 'm') Browser .setBrowserOption ('ContentScale', 2);
   if (k == 'n') Browser .setBrowserOption ('ContentScale', 0.5);
   if (k == 'o') Browser .setBrowserOption ('ContentScale', 1);
}""")
write("sg-runtime", sg)
keyscript("sg-runtime", "abcdefghijklmno")

# ---- Viewpoints inside ScreenGroup (also nested), TELEPORT, one pose per step ----------------------------
vw = HEAD + "DEF V1 Viewpoint { position 0 0 8 description \"one\" }\n" + BG + BASE
vw += "Transform { translation 1 2 0 rotation 0 1 0 0.4 children ScreenGroup { children [ DEF VI1 Viewpoint { position 0 0 5 description \"in1\" }\n" + rect(20, 20, "0.9 0.4 0.1") + " ] } }\n"
vw += ("Transform { translation -1 -2 0 children ScreenGroup { children [ ScreenGroup { children [ DEF VI2 Viewpoint { position 1 1 3 orientation 0 1 0 0.5 description \"in2\" }\n"
       + rect(20, 20, "0.2 0.4 0.9") + " ] } ] } }\n")
vw += script_node(refs(["V1", "VI1", "VI2"]), """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') vi1 .set_bind = true;
   if (k == 'b') v1 .set_bind = true;
   if (k == 'c') vi2 .set_bind = true;
   if (k == 'd') vi1 .position = new SFVec3f (0, 0, 9);
   if (k == 'e') vi1 .set_bind = true;
}""")
write("sg-viewpoint", vw)
keyscript("sg-viewpoint", "abcde")

# ---- ScreenFontStyle: run-time font changes ----------------------------------------------------------
def stext(tr, defs, strings, fs):
    s = " ".join(f'"{x}"' for x in strings)
    return (f"Transform {{ translation {tr} children ScreenGroup {{ children [ Shape {{ appearance {EMIT('1 1 1')} geometry DEF {defs} Text {{ string [ {s} ] "
            f"fontStyle DEF F{defs} ScreenFontStyle {{ {fs} }} solid FALSE }} }} ] }} }}\n")
sf = HEAD + "DEF V1 Viewpoint { position 0 0 10 }\nDEF V2 Viewpoint { position 0 0 6 orientation 0 0 1 0.2 }\n" + BG
sf += stext("-2.5 2 0", "TA", ["Abc"], "pointSize 16")
sf += stext("2.5 2 0", "TB", ["Ab", "Cd"], "pointSize 14")
sf += stext("-2.5 -1 0", "TC", ["Abcdefg"], "pointSize 14")
sf += stext("2.5 -1 0", "TD", ["Run", "time"], "pointSize 18 justify [ \"MIDDLE\" \"MIDDLE\" ]")
sf = sf.replace("DEF FTA ", "DEF FA ").replace("DEF FTB ", "DEF FB ").replace("DEF FTC ", "DEF FC ").replace("DEF FTD ", "DEF FD ")
sf += script_node(refs(["TA", "FA", "TB", "FB", "TC", "FC", "TD", "FD", "V2"]),
"""function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') fa .pointSize = 32;
   if (k == 'b') fa .family = new MFString ('TYPEWRITER');
   if (k == 'c') fa .style = 'BOLDITALIC';
   if (k == 'd') fb .horizontal = false;
   if (k == 'e') fb .justify = new MFString ('END', 'END');
   if (k == 'f') fb .spacing = 2;
   if (k == 'g') tc .length = new MFFloat (100);
   if (k == 'h') tc .maxExtent = 60;
   if (k == 'i') td .string = new MFString ('Changed');
   if (k == 'j') fd .family = new MFString ('NoSuchFont', 'SANS');
   if (k == 'k') v2 .set_bind = true;
   if (k == 'l') Browser .setBrowserOption ('ContentScale', 2);
   if (k == 'm') fa .pointSize = 12;
   if (k == 'n') Browser .setBrowserOption ('ContentScale', 1);
}""")
write("sfs-runtime", sf)
keyscript("sfs-runtime", "abcdefghijklmn", settle=True)

# ---- Text / FontStyle: run-time changes through the node fields ---------------------------------------
tx = HEAD + "DEF V1 Viewpoint { position 0 0 10 }\n" + BG
def ptext(tr, defs, strings, fs, extra=""):
    s = " ".join(f'"{x}"' for x in strings)
    return (f"Transform {{ translation {tr} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.95 0.95 0.95 }} }} geometry DEF {defs} Text {{ string [ {s} ] "
            f"fontStyle DEF F{defs} FontStyle {{ {fs} }} {extra} solid FALSE }} }} }}\n")
tx += ptext("-3 2 0", "TA", ["Abc"], "size 1")
tx += ptext("3 2 0", "TB", ["Ab", "Cd"], "size 0.8")
tx += ptext("-3 -1 0", "TC", ["Abcdefg"], "size 0.8")
tx += ptext("3 -1 0", "TD", ["Run", "time"], "size 0.9 justify [ \"MIDDLE\" \"MIDDLE\" ]")
tx += script_node(refs(["TA", "FTA", "TB", "FTB", "TC", "TD", "FTD"]),
"""function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') fta .size = 1.6;
   if (k == 'b') fta .family = new MFString ('TYPEWRITER');
   if (k == 'c') fta .style = 'BOLDITALIC';
   if (k == 'd') ftb .horizontal = false;
   if (k == 'e') ftb .justify = new MFString ('END', 'END');
   if (k == 'f') ftb .spacing = 2;
   if (k == 'g') tc .length = new MFFloat (3);
   if (k == 'h') tc .maxExtent = 1.5;
   if (k == 'i') td .string = new MFString ('Changed');
   if (k == 'j') ftd .family = new MFString ('NoSuchFont', 'SANS');
   if (k == 'k') ftd .leftToRight = false;
   if (k == 'l') td .fontStyle = new SFNode (null);
}""")
write("text-runtime", tx)
keyscript("text-runtime", "abcdefghijkl", settle=True)

open(os.path.join(root, "cases.txt"), "w").write("".join(f"{n} oracle/synthetic-rea-b10-stepped/{n}.wrl oracle/synthetic-rea-b10-stepped/{n}.json\n" for n in cases))
print(cases)
