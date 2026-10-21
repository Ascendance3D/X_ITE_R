#!/usr/bin/env python3
"""REA batch 12 view lane (node:OrthoViewpoint's open items, shared viewpoint code): stepped worlds + key / mouse scripts in
oracle/synthetic-rea-b12-view. 512x512, dt = 1/30, KeySensor driven like gen_rea_b11_ortho.py.

  - X3DBrowser.viewAll (transitionTime default, 0.5, 0, one number argument, a Layer node, junk argument), with the
    StraightenHorizon option TRUE and FALSE, after run-time changes of the viewpoint's orientation and its parent Transform.
  - Pose events: a ProximitySensor around the world reports position_changed / orientation_changed / centerOfRotation_changed,
    the viewpoints report isBound / bindTime; a Script stamps each with the key and the frames since it. A pose event is recorded
    when a component moved more than 1e-6 since the last recorded one (ProximitySensor sends an event for any change of the double
    value; a transition between rotations that differ only by float32 rounding moves them by about 1e-8), printed with six
    decimals; the cases compare these numbers with the pose bound 1e-4 (cases.txt text_tol).
  - LookAt on a shape (lookAtBBox) and with Alt (lookAtPoint), StraightenHorizon TRUE and FALSE.
  - The same for a perspective Viewpoint and a GeoViewpoint (shared X3DViewpointNode code), and a LayerSet with two layers.

  python3 -I -B oracle/tools/gen_rea_b12_view.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b12-view"))
os.makedirs(root, exist_ok=True)
HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
SOLID = lambda c: f"Appearance {{ material Material {{ diffuseColor {c} }} }}"

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def script(name, frames, events, capture):
    d = {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": sorted(events, key=lambda e: e["frame"]), "buildEachFrame": True}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)

def keys(letters, step, start):
    return [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]

def click(frame, x, y, alt=False):
    ev = [{"frame": frame, "type": "mousedown", "button": 0, "x": x, "y": y}, {"frame": frame + 1, "type": "mouseup", "button": 0, "x": x, "y": y}]
    if alt:
        ev = [{"frame": frame, "type": "mods", "alt": True}] + ev + [{"frame": frame + 1, "type": "mods", "alt": False}]
    return ev

PS = "DEF PS ProximitySensor { size 10000 10000 10000 }\n"

# The stamps: a ProximitySensor (PS, placed by the world) around everything and the viewpoints' bind outputs; `out` takes the events so far at the start of
# each frame (prepareEvents), so each dump carries the events up to the frame before. Events are stamped `k<key>+<frames since that
# key>` from the first key on: the frame count of the oracle's warm-up (the frames before the world counted as loaded) is not part of
# the comparison (see rea_batch11_text.rs).
def stamper(viewpoints, actions, others=()):
    fields = "".join(f"  inputOnly SFBool b_{v}\n  inputOnly SFTime t_{v}\n" for v in viewpoints)
    funcs = "".join(f"function b_{v} (v) {{ rec ('{v}.isBound', v); }}\nfunction t_{v} (v) {{ rec ('{v}.bindTime', 'T'); }}\n" for v in viewpoints)
    routes = "".join(f"ROUTE {v}.isBound TO R.b_{v}\nROUTE {v}.bindTime TO R.t_{v}\n" for v in viewpoints)
    refs = "".join(f"  initializeOnly SFNode {v.lower()} USE {v}\n" for v in list(viewpoints) + list(others))
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n"
            "  inputOnly SFString on_key\n  inputOnly SFVec3f pos\n  inputOnly SFRotation ori\n  inputOnly SFVec3f cor\n" + fields + refs
            + "  inputOutput MFString out [ ]\n  url \"javascript:\nvar K_ = -1, F_ = 0, R_ = [], L_ = { };\n"
            "function nums (v) { return String (v) .split (/\\\\s+/) .map (Number); }\n"
            "function rec (n, v) { if (K_ >= 0) R_ .push ('k' + K_ + '+' + F_ + ' ' + n + '=' + v); }\n"
            "function prepareEvents () { ++ F_; if (R_ .length !== out .length) out = new MFString (... R_); }\n"
            "function pose (n, v) { v = nums (v); const l = L_ [n]; if (l && v .every ((x, i) => Math .abs (x - l [i]) <= 1e-6)) return; L_ [n] = v; rec (n, v .map (x => x .toFixed (6)) .join (' ')); }\n"
            "function pos (v) { pose ('position', v); }\nfunction ori (v) { pose ('orientation', v); }\n"
            "function cor (v) { pose ('centerOfRotation', v); }\n" + funcs
            + "function on_key (k)\n{\n   ++ K_; F_ = 0;\n   rec ('key', k);\n" + actions + "}\n\"\n}\nROUTE K.keyPress TO R.on_key\n"
            "ROUTE PS.position_changed TO R.pos\nROUTE PS.orientation_changed TO R.ori\nROUTE PS.centerOfRotation_changed TO R.cor\n" + routes)

SCENE = ("DEF G Transform { rotation 0 1 0 0.4 children [\n"
         f"  Transform {{ translation -1.5 0 0 rotation 1 0 0 0.5 children Shape {{ appearance {SOLID('0.3 0.5 0.9')} geometry Box {{ size 1.5 1.5 1.5 }} }} }}\n"
         f"  Transform {{ translation 1.5 0.3 0 children Shape {{ appearance {SOLID('0.9 0.6 0.2')} geometry Sphere {{ radius 0.9 }} }} }}\n"
         f"  Transform {{ translation 0 2.5 -2 children Shape {{ appearance {SOLID('0.4 0.9 0.4')} geometry Box {{ size 1 1 1 }} }} }}\n"
         "] }\n")

# Browser.viewAll in every argument form, StraightenHorizon on and off, viewpoint orientation and parent changes at run time.
ACTIONS = """   if (k == 'a') Browser .viewAll ();
   if (k == 'b') Browser .viewAll (0.5);
   if (k == 'c') Browser .viewAll (0);
   if (k == 'd') { Browser .setBrowserOption ('StraightenHorizon', false); Browser .viewAll (0.4); }
   if (k == 'e') { vp .orientation = new SFRotation (0.2, 0.1, 1, 0.5); }
   if (k == 'f') Browser .viewAll (null, 0.4);
   if (k == 'g') { Browser .setBrowserOption ('StraightenHorizon', true); Browser .viewAll (0.4); }
   if (k == 'h') { pt .rotation = new SFRotation (0, 0, 1, -0.3); pt .translation = new SFVec3f (0.4, -0.2, 0); Browser .viewAll (0.3); }
   if (k == 'i') Browser .viewAll ('x', 0.2);
   if (k == 'j') Browser .viewAll (Browser .currentScene .getNamedNode ('G'), 0.2);
"""

def viewworld(name, viewpoint, geo=False):
    head = HEAD + ("COMPONENT Geospatial:1\n" if geo else "")
    # A GeoViewpoint's user position is geocentric: the ProximitySensor sits in the GeoLocation with the shapes.
    scene = f"GeoLocation {{ geoCoords 0 0 0 children [\n{SCENE}{PS}] }}\n" if geo else SCENE + PS
    body = (head + "Background { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
            + f"DEF PT Transform {{ translation 0.5 0.2 0 rotation 0 1 0 0.3 children DEF VP {viewpoint} }}\n" + scene
            + stamper(["VP"], ACTIONS, ["PT"]))
    write(name, body)
    ev = keys("abcdefghij", 26, 6)
    # LookAt: a shape (bbox), Alt for the point, then StraightenHorizon off (key d) once more and the same clicks.
    ev += [{"frame": 272, "type": "viewer", "name": "LOOKAT"}]
    ev += click(274, 300, 260) + click(304, 200, 250, alt=True) + [{"frame": 334, "type": "key", "key": "d"}] + click(364, 256, 300) + click(394, 330, 240, alt=True)
    ev += [{"frame": 424, "type": "key", "key": "g"}]
    capture = [4] + [6 + 26 * i + 3 for i in range(10)] + [6 + 26 * i + 20 for i in range(10)] + [290, 320, 350, 380, 410, 440, 455]
    script(name, 460, ev, sorted(capture))

viewworld("vw-ortho", "OrthoViewpoint { position 0 0 10 orientation 1 0 0 -0.25 fieldOfView [ -3 -3 3 3 ] }")
viewworld("vw-persp", "Viewpoint { position 0 0 10 orientation 1 0 0 -0.25 fieldOfView 0.7 }")
viewworld("vw-geo", "GeoViewpoint { position 0 0 10 orientation 1 0 0 -1.4 fieldOfView 0.7 }", geo=True)

# Two layers: Browser.viewAll on the second layer's node, on the active layer (the LayerSet's activeLayer), with one number. The
# ProximitySensor is in the second layer (it reports that layer's camera).
LAYER = lambda v, c: (f"Layer {{ children [ DEF {v} OrthoViewpoint {{ position 0 0 10 fieldOfView [ -3 -3 3 3 ] }}\n"
                      f"  Transform {{ translation 1 0.5 0 children Shape {{ appearance {SOLID(c)} geometry Box {{ size 1 2 1 }} }} }}\n"
                      f"  Transform {{ translation -2 -1 0 children Shape {{ appearance {SOLID(c)} geometry Sphere {{ radius 0.5 }} }} }}{' ' + PS if v == 'V2' else ''} ] }}")
write("vw-layers", HEAD + "Background { skyColor 0.12 0.14 0.2 }\n"
      + f"DEF LS LayerSet {{ activeLayer 1 order [ 1 2 ] layers [ DEF L1 {LAYER('V1', '0.9 0.6 0.2')}\n DEF L2 {LAYER('V2', '0.3 0.5 0.9')} ] }}\n"
      + stamper(["V1", "V2"], """   if (k == 'a') Browser .viewAll (Browser .currentScene .getNamedNode ('L2'), 0.4);
   if (k == 'b') Browser .viewAll (0.4);
   if (k == 'c') { ls .activeLayer = 2; }
   if (k == 'd') Browser .viewAll (0.4);
   if (k == 'e') Browser .viewAll (Browser .currentScene .getNamedNode ('L1'), 0);
""", ["LS"]))
script("vw-layers", 150, keys("abcde", 26, 6), [4] + [6 + 26 * i + 3 for i in range(5)] + [6 + 26 * i + 20 for i in range(5)])
