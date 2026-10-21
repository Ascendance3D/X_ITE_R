#!/usr/bin/env python3
"""REA batch 09 lane render-inputs: stepped (run-time) worlds and event scripts for Fog / LocalFog /
FogCoordinate, Background, Viewpoint, Billboard, Collision and Anchor (oracle/synthetic-rea-b09-stepped).
A KeySensor drives a Script that changes the live scene; the scripts press keys and, for Collision and
Anchor, hold / click the pointer. 512x512, dt = 1/30. cases.txt lists `name world script`.

  python3 -I oracle/tools/gen_rea_b09_stepped.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b09-stepped"))
os.makedirs(root, exist_ok=True)
cases = []
HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
LIGHT = "DirectionalLight { direction -0.3 -0.4 -1 intensity 0.9 ambientIntensity 0.3 }\n"
MAT = "Material { diffuseColor 0.8 0.6 0.4 specularColor 0.4 0.3 0.2 shininess 0.3 }"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, world, frames, events, capture, dump=None):
    ev = sorted(events, key=lambda e: e["frame"])
    d = {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": dump if dump is not None else sorted(set(capture + [frames - 1])), "events": ev}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)
    cases.append((name, world, name))

def key(frame, k):
    return {"frame": frame, "type": "key", "key": k}

def keys(start, step, letters):
    return [key(start + step * i, c) for i, c in enumerate(letters)]

def keyscript(name, world, letters, step=8, start=6, tail=6, extra=None):
    """Press `letters` every `step` frames; capture 3 frames after each press."""
    ev = keys(start, step, letters) + (extra or [])
    frames = start + step * len(letters) + tail
    capture = [start - 2] + [start + step * i + 3 for i in range(len(letters))]
    script(name, world, frames, ev, capture)

def sphere(x, y, z, r=0.8, mat=MAT):
    return f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material {mat} }} geometry Sphere {{ radius {r} }} }} }}\n"

KEYSENSOR = "DEF K KeySensor { }\n"

def script_node(fields, body, logs=""):
    return ("DEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + fields + "\n  url \"javascript:\n" + body + "\n\"\n}\nROUTE K.keyPress TO R.on_key\n" + logs)

# ---- Fog / LocalFog / FogCoordinate ---------------------------------------------------------------
fog = HEAD + "Viewpoint { position 0 0 6 }\nNavigationInfo { type \"NONE\" headlight FALSE }\nDEF B Background { skyColor 0.2 0.25 0.3 }\n" + LIGHT
fog += "DEF F1 Fog { color 0.8 0.4 0.3 visibilityRange 14 }\nDEF F2 Fog { color 0.2 0.8 0.5 fogType \"EXPONENTIAL\" visibilityRange 8 }\n"
fog += sphere(-3, 0, -2) + sphere(3, 0, -8) + sphere(0, 1.5, -5)
fog += "DEF G Transform { children [ DEF LF LocalFog { color 1 1 0.2 visibilityRange 9 }\n" + sphere(-1.5, -1.5, -1) + sphere(1.5, -1.5, -6) + "] }\n"
fog += f"""DEF H Group {{ }}
DEF FS Transform {{ translation 4.5 0 -1 children Shape {{ appearance Appearance {{ material {MAT} }}
  geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.4 -1.5 0, 0.4 -1.5 0, 0.4 -0.5 0, -0.4 -0.5 0, 0.4 0.5 0, -0.4 0.5 0, 0.4 1.5 0, -0.4 1.5 0 ] }}
    coordIndex [ 0 1 2 3 -1 3 2 4 5 -1 5 4 6 7 -1 ] fogCoord DEF FC FogCoordinate {{ depth [ 0 0 0.3 0.3 0.6 0.6 1 1 ] }} }} }} }}
"""
fog += KEYSENSOR
fog += script_node("""  initializeOnly SFNode f1 USE F1
  initializeOnly SFNode f2 USE F2
  initializeOnly SFNode lf USE LF
  initializeOnly SFNode g USE G
  initializeOnly SFNode fc USE FC
  initializeOnly SFNode b USE B
  initializeOnly SFNode h USE H
  inputOnly SFBool on_b1
  inputOnly SFBool on_b2
  inputOnly SFTime on_t1""", """var lf2 = null;
function on_b1 (v) { log = log + 'F1:' + v + ' '; }
function on_b2 (v) { log = log + 'F2:' + v + ' '; }
function on_t1 (v) { log = log + 'T1 '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') f1 .color = new SFColor (0.2, 0.3, 0.9);
   if (k == 'b') f1 .visibilityRange = 6;
   if (k == 'c') f1 .fogType = 'EXPONENTIAL';
   if (k == 'd') f2 .set_bind = true;
   if (k == 'e') f1 .set_bind = true;
   if (k == 'f') lf .enabled = false;
   if (k == 'g') lf .enabled = true;
   if (k == 'h') lf .color = new SFColor (0.1, 1, 0.3);
   if (k == 'i') lf .visibilityRange = 0;
   if (k == 'j') lf .visibilityRange = 5;
   if (k == 'k') lf .fogType = 'EXPONENTIAL';
   if (k == 'l') lf .visibilityStart = 2;
   if (k == 'm') fc .depth = new MFFloat (1, 1, 0.5, 0.5, 0, 0, 0.5, 0.5);
   if (k == 'n') g .scale = new SFVec3f (2, 2, 2);
   if (k == 'o') g .removeChildren = new MFNode (lf);
   if (k == 'p') g .addChildren = new MFNode (lf);
   if (k == 'q') f2 .set_bind = false;
   if (k == 'r') f1 .visibilityRange = 0;
   if (k == 's') f1 .visibilityRange = 14;
   if (k == 't') { lf2 = Browser .currentScene .createNode ('LocalFog'); lf2 .color = new SFColor (1, 0.2, 0.2); lf2 .visibilityRange = 7; h .addChildren = new MFNode (lf2); }
   if (k == 'u') lf2 .enabled = false;
   if (k == 'v') fc .depth = new MFFloat ();
   if (k == 'w') g .translation = new SFVec3f (0, 1, 3);
}""", "ROUTE F1.isBound TO R.on_b1\nROUTE F2.isBound TO R.on_b2\nROUTE F1.bindTime TO R.on_t1\n")
write("fog-runtime", fog)
keyscript("fog-runtime", "fog-runtime", "abcdeqfghijklmnopstuvrsw", step=7)

# ---- Background ------------------------------------------------------------------------------------
bg = HEAD + "Viewpoint { position 0 0 6 }\nNavigationInfo { type \"NONE\" headlight FALSE }\n" + LIGHT
bg += "DEF B1 Background { skyColor [ 0.1 0.2 0.8, 0.8 0.8 1 ] skyAngle [ 1.2 ] groundColor [ 0.3 0.2 0.1, 0.1 0.1 0.1 ] groundAngle [ 1.4 ] }\n"
bg += "DEF B2 Background { skyColor [ 0.9 0.3 0.1 ] transparency 0 }\n"
bg += "DEF B3 Background { backUrl \"../synthetic-rea-b03/proj-checker.png\" frontUrl \"../synthetic-rea-b03/proj-checker.png\" leftUrl \"../synthetic-rea-b03/proj-checker.png\" rightUrl \"../synthetic-rea-b03/proj-checker.png\" topUrl \"../synthetic-rea-b03/proj-checker.png\" bottomUrl \"../synthetic-rea-b03/proj-checker.png\" skyColor 0.2 0.8 0.3 }\n"
bg += "DEF TB TextureBackground { frontTexture PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 } skyColor 0.5 0.5 0.5 }\n"
bg += sphere(-1.5, 0, 0) + sphere(1.5, 0, -2)
bg += KEYSENSOR
bg += script_node("""  initializeOnly SFNode b1 USE B1
  initializeOnly SFNode b2 USE B2
  initializeOnly SFNode b3 USE B3
  initializeOnly SFNode tb USE TB
  inputOnly SFBool on_1
  inputOnly SFBool on_2
  inputOnly SFBool on_3
  inputOnly SFBool on_4
  inputOnly SFTime on_t2""", """function on_1 (v) { log = log + 'B1:' + v + ' '; }
function on_2 (v) { log = log + 'B2:' + v + ' '; }
function on_3 (v) { log = log + 'B3:' + v + ' '; }
function on_4 (v) { log = log + 'TB:' + v + ' '; }
function on_t2 (v) { log = log + 'T2 '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') b1 .skyColor = new MFColor (new SFColor (0.9, 0.1, 0.1), new SFColor (0.1, 0.9, 0.1));
   if (k == 'b') b1 .skyAngle = new MFFloat (0.6);
   if (k == 'c') b1 .groundColor = new MFColor (new SFColor (0.6, 0.6, 0.1));
   if (k == 'd') b1 .groundAngle = new MFFloat ();
   if (k == 'e') b1 .transparency = 0.5;
   if (k == 'f') b2 .set_bind = true;
   if (k == 'g') b2 .skyColor = new MFColor (new SFColor (0.1, 0.1, 0.1));
   if (k == 'h') b3 .set_bind = true;
   if (k == 'i') b3 .transparency = 0.6;
   if (k == 'j') b3 .backUrl = new MFString ();
   if (k == 'k') tb .set_bind = true;
   if (k == 'l') tb .frontTexture = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'm') { var t = tb .frontTexture; t .image = new SFImage (1, 2, 3, new MFInt32 (0x00ffff, 0xff00ff)); }
   if (k == 'n') tb .set_bind = false;
   if (k == 'o') b3 .set_bind = false;
   if (k == 'p') b2 .set_bind = false;
   if (k == 'q') b1 .set_bind = false;
   if (k == 'r') b1 .set_bind = true;
   if (k == 's') b1 .skyColor = new MFColor ();
}""", "ROUTE B1.isBound TO R.on_1\nROUTE B2.isBound TO R.on_2\nROUTE B3.isBound TO R.on_3\nROUTE TB.isBound TO R.on_4\nROUTE B2.bindTime TO R.on_t2\n")
write("background-runtime", bg)
keyscript("background-runtime", "background-runtime", "abcdefghijklmnopqrs", step=7)

# ---- Viewpoint -------------------------------------------------------------------------------------
vp = HEAD + "NavigationInfo { type \"NONE\" headlight FALSE transitionType [ \"LINEAR\" ] transitionTime 0.3 }\nBackground { skyColor 0.2 0.25 0.3 }\n" + LIGHT
vp += "DEF V1 Viewpoint { position 0 0 8 description \"first\" }\nDEF V2 Viewpoint { position 4 2 6 orientation 0 1 0 0.5 description \"second\" fieldOfView 1.2 }\n"
vp += "DEF V3 Viewpoint { position -4 3 6 orientation 0 1 0 -0.4 description \"third\" jump FALSE centerOfRotation 1 0 0 }\n"
vp += "DEF V4 Viewpoint { position 0 0 10 description \"fourth\" retainUserOffsets TRUE }\nDEF OV OrthoViewpoint { position 0 0 8 fieldOfView [ -3 -3 3 3 ] description \"ortho\" }\n"
vp += sphere(-1.5, 0, 0) + sphere(1.5, 0.5, -2) + sphere(0, -1, -5, 1.2)
vp += KEYSENSOR
vp += script_node("""  initializeOnly SFNode v1 USE V1
  initializeOnly SFNode v2 USE V2
  initializeOnly SFNode v3 USE V3
  initializeOnly SFNode v4 USE V4
  initializeOnly SFNode ov USE OV
  inputOnly SFBool on_1
  inputOnly SFBool on_2
  inputOnly SFBool on_3
  inputOnly SFBool on_4
  inputOnly SFBool on_o
  inputOnly SFTime on_t3
  inputOnly SFVec3f on_c""", """function on_1 (v) { log = log + 'V1:' + v + ' '; }
function on_2 (v) { log = log + 'V2:' + v + ' '; }
function on_3 (v) { log = log + 'V3:' + v + ' '; }
function on_4 (v) { log = log + 'V4:' + v + ' '; }
function on_o (v) { log = log + 'OV:' + v + ' '; }
function on_t3 (v) { log = log + 'T3 '; }
function on_c (v) { log = log + 'C' + v + ' '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') v2 .set_bind = true;
   if (k == 'b') v3 .set_bind = true;
   if (k == 'c') v3 .position = new SFVec3f (-2, 4, 5);
   if (k == 'd') v3 .orientation = new SFRotation (1, 0, 0, -0.3);
   if (k == 'e') v3 .centerOfRotation = new SFVec3f (0, 1, 0);
   if (k == 'f') v3 .fieldOfView = 0.6;
   if (k == 'g') v3 .set_bind = false;
   if (k == 'h') v1 .set_bind = true;
   if (k == 'i') v4 .set_bind = true;
   if (k == 'j') v4 .position = new SFVec3f (2, 1, 9);
   if (k == 'k') ov .set_bind = true;
   if (k == 'l') ov .fieldOfView = new MFFloat (-1.5, -1.5, 1.5, 1.5);
   if (k == 'm') ov .position = new SFVec3f (1, 0, 8);
   if (k == 'n') ov .set_bind = false;
   if (k == 'o') v2 .jump = false;
   if (k == 'p') v2 .set_bind = true;
   if (k == 'q') v1 .description = 'renamed';
   if (k == 'r') v1 .set_bind = false;
   if (k == 's') v4 .set_bind = false;
}""", "ROUTE V1.isBound TO R.on_1\nROUTE V2.isBound TO R.on_2\nROUTE V3.isBound TO R.on_3\nROUTE V4.isBound TO R.on_4\nROUTE OV.isBound TO R.on_o\nROUTE V2.bindTime TO R.on_t3\nROUTE V3.centerOfRotation_changed TO R.on_c\n")
write("viewpoint-runtime", vp)
keyscript("viewpoint-runtime", "viewpoint-runtime", "abcdefghijklmnopqrs", step=14)

# ---- Billboard -------------------------------------------------------------------------------------
def card(color):
    return f"Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} emissiveColor {color} }} }} geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.6 -0.4 0, 0.6 -0.4 0, 0.6 0.4 0, 0 0.7 0, -0.6 0.4 0 ] }} coordIndex [ 0 1 2 3 4 -1 ] }} }}"
bb = HEAD + "NavigationInfo { type \"NONE\" headlight FALSE transitionType [ \"TELEPORT\" ] }\nBackground { skyColor 0.2 0.25 0.3 }\n" + LIGHT
bb += "DEF V1 Viewpoint { position 0 0 8 }\nDEF V2 Viewpoint { position 6 0 4 orientation 0 1 0 1.0 }\nDEF V3 Viewpoint { position 0 7 5 orientation 1 0 0 -1.0 }\nDEF V4 Viewpoint { position -5 -3 5 orientation 0.3 -1 0 -0.8 }\n"
bb += f"DEF BA Transform {{ translation -3 1.5 0 children DEF B1 Billboard {{ axisOfRotation 0 1 0 children {card('1 0.3 0.3')} }} }}\n"
bb += f"DEF BB Transform {{ translation 0 1.5 0 children DEF B2 Billboard {{ axisOfRotation 0 0 0 children {card('0.3 1 0.3')} }} }}\n"
bb += f"DEF BC Transform {{ translation 3 1.5 0 children DEF B3 Billboard {{ axisOfRotation 1 0 0 children {card('0.3 0.3 1')} }} }}\n"
bb += f"DEF BD Transform {{ translation -3 -1.5 0 rotation 0 0 1 0.6 scale 1.3 1 1 children DEF B4 Billboard {{ axisOfRotation 1 1 1 children {card('1 1 0.3')} }} }}\n"
bb += f"DEF BE Transform {{ translation 0 -1.5 0 rotation 1 0 0 0.9 children DEF B5 Billboard {{ axisOfRotation 0 1 0 children [ {card('0.3 1 1')} Transform {{ translation 1 0 0 rotation 0 1 0 1 children {card('1 0.3 1')} }} ] }} }}\n"
bb += f"DEF BF Transform {{ translation 3 -1.5 0 children DEF B6 Billboard {{ axisOfRotation 0 -1 0 children {card('0.9 0.6 0.2')} }} }}\n"
bb += KEYSENSOR
bb += script_node("""  initializeOnly SFNode v2 USE V2
  initializeOnly SFNode v3 USE V3
  initializeOnly SFNode v4 USE V4
  initializeOnly SFNode b1 USE B1
  initializeOnly SFNode b2 USE B2
  initializeOnly SFNode b3 USE B3
  initializeOnly SFNode b5 USE B5
  initializeOnly SFNode ba USE BA
  initializeOnly SFNode be USE BE""", """function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') v2 .set_bind = true;
   if (k == 'b') v3 .set_bind = true;
   if (k == 'c') v4 .set_bind = true;
   if (k == 'd') b1 .axisOfRotation = new SFVec3f (0, 0, 1);
   if (k == 'e') b2 .axisOfRotation = new SFVec3f (0, 1, 0);
   if (k == 'f') b3 .axisOfRotation = new SFVec3f (0, 0, 0);
   if (k == 'g') ba .rotation = new SFRotation (0, 0, 1, 0.7);
   if (k == 'h') be .rotation = new SFRotation (0, 1, 0, 0.8);
   if (k == 'i') b5 .axisOfRotation = new SFVec3f (0, 0, 0);
   if (k == 'j') ba .scale = new SFVec3f (1, 2, 1);
   if (k == 'k') b1 .axisOfRotation = new SFVec3f (0, 2, 0);
   if (k == 'l') v2 .position = new SFVec3f (-6, 0, 4);
}""")
write("billboard-runtime", bb)
keyscript("billboard-runtime", "billboard-runtime", "abcdefghijkl", step=8)

# ---- Collision -------------------------------------------------------------------------------------
def wall(x, y, z, sx, sy, sz, c="0.8 0.3 0.3"):
    return f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor {c} }} }} geometry Box {{ size {sx} {sy} {sz} }} }} }}"
co = "#X3D V4.0 utf8\nPROFILE Full\nNavigationInfo { type [ \"WALK\" ] avatarSize [ 0.25 1.6 0.75 ] speed 4 }\nBackground { skyColor 0.2 0.25 0.3 }\n" + LIGHT
co += "Viewpoint { position 0 1.6 12 }\n"
co += wall(0, -0.1, 0, 40, 0.2, 40, "0.4 0.4 0.4") + "\n"
co += "DEF C1 Collision { enabled TRUE children [ " + wall(0, 1.5, 9, 8, 3, 0.4) + " ] }\n"
co += "DEF C2 Collision { enabled TRUE proxy Shape { geometry Box { size 8 3 0.4 } } children [ " + wall(0, 1.5, 6, 8, 3, 0.4, "0.3 0.8 0.3") + " ] }\n"
co += "DEF C3 Collision { enabled TRUE proxy Group { } children [ " + wall(0, 1.5, 3, 8, 3, 0.4, "0.3 0.3 0.8") + " ] }\n"
co += "DEF C4 Collision { enabled TRUE children [ DEF C5 Collision { enabled FALSE children [ " + wall(0, 1.5, 0, 8, 3, 0.4, "0.8 0.8 0.3") + " ] } ] }\n"
co += wall(5, 1, 8, 1, 2, 1, "0.9 0.9 0.9")
co += KEYSENSOR
co += script_node("""  initializeOnly SFNode c1 USE C1
  initializeOnly SFNode c2 USE C2
  initializeOnly SFNode c3 USE C3
  initializeOnly SFNode c4 USE C4
  initializeOnly SFNode c5 USE C5
  inputOnly SFBool on_a1
  inputOnly SFTime on_ct1
  inputOnly SFBool on_a2
  inputOnly SFTime on_ct2
  inputOnly SFBool on_a3
  inputOnly SFBool on_a4
  inputOnly SFBool on_a5""", """function on_a1 (v) { log = log + 'C1:' + v + ' '; }
function on_ct1 (v) { log = log + 'T1:' + (v - 946684800) + ' '; }
function on_a2 (v) { log = log + 'C2:' + v + ' '; }
function on_ct2 (v) { log = log + 'T2:' + (v - 946684800) + ' '; }
function on_a3 (v) { log = log + 'C3:' + v + ' '; }
function on_a4 (v) { log = log + 'C4:' + v + ' '; }
function on_a5 (v) { log = log + 'C5:' + v + ' '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') c1 .enabled = false;
   if (k == 'b') c1 .enabled = true;
   if (k == 'c') c2 .enabled = false;
   if (k == 'd') c2 .enabled = true;
   if (k == 'e') c5 .enabled = true;
   if (k == 'f') c4 .enabled = false;
}""", "ROUTE C1.isActive TO R.on_a1\nROUTE C1.collideTime TO R.on_ct1\nROUTE C2.isActive TO R.on_a2\nROUTE C2.collideTime TO R.on_ct2\nROUTE C3.isActive TO R.on_a3\nROUTE C4.isActive TO R.on_a4\nROUTE C5.isActive TO R.on_a5\n")
write("collision-runtime", co)
def hold(button, start, to, frames, f0):
    return [{"frame": f0, "type": "mousedown", "button": button, "x": start[0], "y": start[1]}, {"frame": f0 + 1, "type": "mousemove", "x": to[0], "y": to[1]},
            {"frame": f0 + 1 + frames, "type": "mouseup", "button": button, "x": to[0], "y": to[1]}]
# walk forward through C1 (blocked), disable it, walk on (through), re-enable; later C2 / C3 / C4
ev = hold(0, (256, 256), (256, 340), 158, 2) + [key(40, "a"), key(70, "c"), key(95, "e"), key(110, "f"), key(120, "b"), key(135, "d")]
script("collision-walk", "collision-runtime", 170, ev, [30, 38, 44, 60, 68, 74, 90, 100, 108, 114, 125, 140, 169])
ev = hold(0, (256, 256), (256, 340), 60, 2) + [key(70, "a")] + hold(0, (256, 256), (256, 340), 50, 72)
script("collision-walk-disabled", "collision-runtime", 130, ev, [20, 40, 62, 75, 100, 129])

# ---- Anchor ----------------------------------------------------------------------------------------
def box(x, y, z, c):
    return f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor {c} emissiveColor {c} }} }} geometry Box {{ size 1.2 1.2 1.2 }} }} }}"
an = HEAD + "NavigationInfo { type \"NONE\" headlight FALSE transitionType [ \"TELEPORT\" ] }\nBackground { skyColor 0.2 0.25 0.3 }\n" + LIGHT
an += "DEF V1 Viewpoint { position 0 0 8 description \"home\" }\nDEF V2 Viewpoint { position 5 3 6 orientation 0 1 0 0.6 description \"there\" }\nDEF V3 Viewpoint { position -5 -3 6 orientation 0 1 0 -0.6 description \"elsewhere\" }\n"
an += "DEF A1 Anchor { url [ \"#V2\" ] description \"to V2\" children " + box(-2.5, 1.5, 0, "0.8 0.2 0.2") + " }\n"
an += "DEF A2 Anchor { url [ \"#V3\" ] description \"to V3\" load FALSE children " + box(0, 1.5, 0, "0.2 0.8 0.2") + " }\n"
an += "DEF A3 Anchor { url [ \"#nosuch\", \"#V3\" ] description \"fallback\" children " + box(2.5, 1.5, 0, "0.2 0.2 0.8") + " }\n"
an += "DEF A4 Anchor { url [ \"#V1\" ] description \"home\" children [ " + box(-2.5, -1.5, 0, "0.8 0.8 0.2") + " DEF TS TouchSensor { } ] }\n"
an += "DEF A5 Anchor { url [ ] description \"no url\" children " + box(0, -1.5, 0, "0.2 0.8 0.8") + " }\n"
an += "DEF A6 Anchor { url [ \"#V2\" ] description \"hidden\" visible FALSE children " + box(2.5, -1.5, 0, "0.8 0.2 0.8") + " }\n"
an += KEYSENSOR
an += script_node("""  initializeOnly SFNode v1 USE V1
  initializeOnly SFNode a2 USE A2
  initializeOnly SFNode a5 USE A5
  initializeOnly SFNode a6 USE A6
  inputOnly SFBool on_1
  inputOnly SFBool on_2
  inputOnly SFBool on_3
  inputOnly SFBool on_ov
  inputOnly SFTime on_tt""", """function on_1 (v) { log = log + 'V1:' + v + ' '; }
function on_2 (v) { log = log + 'V2:' + v + ' '; }
function on_3 (v) { log = log + 'V3:' + v + ' '; }
function on_ov (v) { log = log + 'over:' + v + ' '; }
function on_tt (v) { log = log + 'touch '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') a2 .load = true;
   if (k == 'b') a5 .url = new MFString ('#V3');
   if (k == 'c') a6 .visible = true;
   if (k == 'd') a2 .url = new MFString ('#V1');
   if (k == 'e') a2 .load = false;
   if (k == 'z') v1 .set_bind = true;
}""", "ROUTE V1.isBound TO R.on_1\nROUTE V2.isBound TO R.on_2\nROUTE V3.isBound TO R.on_3\nROUTE TS.isOver TO R.on_ov\nROUTE TS.touchTime TO R.on_tt\n")
write("anchor-runtime", an)
# pixel positions of the boxes at z = 8, fov pi/4: 512 / (2 * 8 * tan (pi/8)) = 77.25 px per unit
def px(x, y):
    s = 512 / (2 * 8 * 0.41421356)
    return [round(256 + x * s), round(256 + y * s)]
def click(frame, p, move=1):
    return [{"frame": frame, "type": "mousemove", "x": p[0], "y": p[1]}, {"frame": frame + move, "type": "mousedown", "button": 0, "x": p[0], "y": p[1]}, {"frame": frame + move + 1, "type": "mouseup", "button": 0, "x": p[0], "y": p[1]}]
pA1, pA2, pA3, pA4, pA5, pA6 = px(-2.5, 1.5), px(0, 1.5), px(2.5, 1.5), px(-2.5, -1.5), px(0, -1.5), px(2.5, -1.5)
def step(frame, p):
    # rebind the first viewpoint (teleport), then move and click
    return [key(frame - 2, "z")] + click(frame, p)
ev = step(4, pA1) + step(16, pA2) + step(28, pA3) + step(40, pA4) + step(52, pA5) + step(64, pA6) + [key(76, "a")] + step(78, pA2) + [key(90, "b")] + step(92, pA5)
ev += [key(104, "c")] + step(106, pA6) + [key(118, "d")] + step(120, pA2) + [key(132, "e")] + step(134, pA2)
script("anchor-click", "anchor-runtime", 150, ev, [3, 9, 21, 33, 45, 57, 69, 83, 97, 111, 125, 139, 149])
# hover and drag off: press on an Anchor, release elsewhere (no load); press / release on the same one
ev = [{"frame": 3, "type": "mousemove", "x": pA1[0], "y": pA1[1]}, {"frame": 4, "type": "mousedown", "button": 0, "x": pA1[0], "y": pA1[1]}, {"frame": 6, "type": "mousemove", "x": 20, "y": 20}, {"frame": 8, "type": "mouseup", "button": 0, "x": 20, "y": 20}]
ev += click(20, pA3)
script("anchor-drag-off", "anchor-runtime", 40, ev, [2, 7, 12, 24, 39])

with open(os.path.join(root, "cases.txt"), "w") as f:
    for name, w, s in cases:
        f.write(f"{name} oracle/synthetic-rea-b09-stepped/{w}.wrl oracle/synthetic-rea-b09-stepped/{s}.json\n")
print(len(cases), "cases")
