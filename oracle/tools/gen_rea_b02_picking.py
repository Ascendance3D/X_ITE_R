#!/usr/bin/env python3
"""REA batch 02: LinePickSensor and the picking engine (Components/Picking) fixtures.

Writes stepped worlds (with x10 scripts) into oracle/synthetic-rea-b02. The pick sensors' outputs
are routed to a Script whose inputOutput fields carry them into the scene-text dumps.

    python3 -I oracle/tools/gen_rea_b02_picking.py oracle/synthetic-rea-b02
"""
import json
import os
import sys

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Picking:3
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT Geometry2D:1
"""

LINE = """IndexedLineSet { coord Coordinate { point [ -1 0 0, 1 0 0, 0 -1 0.2, 0 1 0.2 ] } coordIndex [ 0 1 -1 2 3 -1 ] }"""


def recorder(sensors, extra_fields="", extra_functions=""):
    """A Script with, per sensor, the last value of each output and an event count."""
    fields, functions, routes = [], [], []
    for name, geometry in sensors:
        outs = [("isActive", "SFBool", "a"), ("pickedGeometry", "MFNode", "g")]
        if geometry:
            outs += [("pickedPoint", "MFVec3f", "p"), ("pickedNormal", "MFVec3f", "n"), ("pickedTextureCoordinate", "MFVec3f", "t")]
        for out, ty, short in outs:
            key = f"{name}_{short}"
            fields.append(f"  inputOnly {ty} on_{key}")
            fields.append(f"  inputOutput {ty} {key} {'FALSE' if ty == 'SFBool' else '[ ]'}")
            fields.append(f"  inputOutput SFInt32 {key}_count 0")
            log = f" log = log + '{key}' + (value .length === undefined ? (value ? 1 : 0) : value .length) + ' ';" if short in "agp" else ""
            functions.append(f"function on_{key} (value) {{ {key} = value; {key}_count = {key}_count + 1;{log} }}")
            routes.append(f"ROUTE {name}.{out} TO R.on_{key}")
    fields.append("  inputOutput SFString log \"\"")
    return ("DEF R Script {\n  directOutput TRUE\n" + "\n".join(fields) + extra_fields + '\n  url "javascript:\n' + "\n".join(functions) + extra_functions + '\n"\n}\n' + "\n".join(routes) + "\n")


def shape(geometry, color):
    return f"Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} transparency 0.3 }} }} geometry {geometry} }}"


worlds, scripts = {}, {}

# A click on the empty top-left corner gives the canvas the keyboard focus.
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]

# --- picking-sweep: a line sensor moves through targets; every sort order and both intersection
# types. T4 is not a pick target; T5/T6 are at the same distance (QuickSort order).
SENSORS = [("BC", "BOUNDS", "CLOSEST"), ("BA", "BOUNDS", "ALL"), ("BS", "BOUNDS", "ALL_SORTED"), ("BN", "BOUNDS", "ANY"),
           ("GC", "GEOMETRY", "CLOSEST"), ("GA", "GEOMETRY", "ALL"), ("GS", "GEOMETRY", "ALL_SORTED")]

sweep = HEAD + """Viewpoint { position 0 0 16 }
NavigationInfo { headlight TRUE }
DEF T1 Transform { translation -3 0 0 children """ + shape("Box { size 2 2 2 }", "0.9 0.3 0.2") + """ }
DEF T2 Transform { translation -1.5 0.3 0 children """ + shape("Sphere { radius 1 }", "0.2 0.8 0.3") + """ }
DEF T3 Transform { translation 3 0 0 rotation 0 0 1 0.5 children """ + shape("Cone { bottomRadius 1 height 2 }", "0.2 0.3 0.9") + """ }
DEF T4 Transform { translation 1 -2.5 0 children """ + shape("Box { size 1 1 1 }", "0.6 0.6 0.6") + """ }
DEF T5 Transform { translation 5.5 1.4 0 children """ + shape("Box { size 1 1 1 }", "0.8 0.8 0.2") + """ }
DEF T6 Transform { translation 5.5 -1.4 0 children """ + shape("Box { size 1 1 1 }", "0.8 0.2 0.8") + """ }
DEF MOVER Transform { translation -7 0 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF LINE """ + LINE + """ }
""" + "\n".join(f"  DEF {n} LinePickSensor {{ intersectionType \"{i}\" sortOrder \"{o}\" pickingGeometry USE LINE pickTarget [ USE T1 USE T2 USE T3 USE T5 USE T6 ] }}" for n, i, o in SENSORS) + """
] }
DEF CLOCK TimeSensor { cycleInterval 4 loop TRUE }
DEF PATH PositionInterpolator { key [ 0 1 ] keyValue [ -7 0 0, 7 0 0 ] }
ROUTE CLOCK.fraction_changed TO PATH.set_fraction
ROUTE PATH.value_changed TO MOVER.set_translation
""" + recorder([(n, i == "GEOMETRY") for n, i, _ in SENSORS])
worlds["picking-sweep.wrl"] = sweep
dump = list(range(10, 130, 6))
scripts["picking-sweep.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 128, "capture": [20, 50, 80, 110], "dump": dump, "events": []}

# --- picking-groups: PickableGroup objectType / matchCriterion filters, pickable FALSE, "NONE",
# nested groups and a PROTO instance as pick target; runtime changes by key.
GROUP_SENSORS = [("SALL", '"ALL"', "MATCH_ANY"), ("SA", '"A"', "MATCH_ANY"), ("SAB", '"A" "B"', "MATCH_EVERY"), ("SB1", '"B"', "MATCH_ONLY_ONE"), ("SB2", '"B"', "MATCH_EVERY"), ("SC", '"C" "A"', "MATCH_ANY")]

groups = HEAD + """PROTO Holder [ inputOutput SFVec3f at 0 0 0 inputOutput MFNode kids [ ] ] { Transform { translation IS at children IS kids } }
Viewpoint { position 0 0 18 }
DEF GA PickableGroup { objectType [ "A" ] children DEF TA Transform { translation -4 0 0 children """ + shape("Box { size 2 2 2 }", "0.9 0.3 0.2") + """ } }
DEF GB PickableGroup { objectType [ "B" ] children DEF TB Transform { translation -1.5 0 0 children """ + shape("Box { size 2 2 2 }", "0.2 0.8 0.3") + """ } }
DEF GAB PickableGroup { objectType [ "A" "B" ] children DEF TAB Transform { translation 1 0 0 children """ + shape("Sphere { radius 1 }", "0.2 0.3 0.9") + """ } }
DEF GN PickableGroup { objectType [ "NONE" ] children DEF TN Transform { translation 3.5 0 0 children """ + shape("Box { size 2 2 2 }", "0.6 0.6 0.6") + """ } }
DEF GOFF PickableGroup { pickable FALSE children DEF TOFF Transform { translation 6 0 0 children """ + shape("Box { size 2 2 2 }", "0.8 0.8 0.2") + """ } }
DEF GOUT PickableGroup { objectType [ "A" ] children DEF GIN PickableGroup { objectType [ "B" ] children DEF TIN Transform { translation -1.5 -3 0 children """ + shape("Box { size 1.5 1.5 1.5 }", "0.8 0.2 0.8") + """ } } }
DEF HP Holder { at 1 -3 0 kids [ """ + shape("Cylinder { radius 0.8 height 1.5 }", "0.3 0.8 0.8") + """ ] }
DEF MOVER Transform { translation -7 0 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF LINE """ + LINE + """ }
""" + "\n".join(f"  DEF {n} LinePickSensor {{ objectType [ {t} ] matchCriterion \"{m}\" sortOrder \"ALL\" pickingGeometry USE LINE pickTarget [ USE TA USE TB USE TAB USE TN USE TOFF USE GIN USE HP ] }}" for n, t, m in GROUP_SENSORS) + """
] }
DEF CLOCK TimeSensor { cycleInterval 4 loop TRUE }
DEF PATH PositionInterpolator { key [ 0 0.5 0.5001 1 ] keyValue [ -7 0 0, 8 0 0, -4 -3 0, 4 -3 0 ] }
ROUTE CLOCK.fraction_changed TO PATH.set_fraction
ROUTE PATH.value_changed TO MOVER.set_translation
DEF K KeySensor { }
""" + recorder([(n, False) for n, _, _ in GROUP_SENSORS], extra_fields="""
  inputOnly SFString on_key
  initializeOnly SFNode sa USE SA
  initializeOnly SFNode sall USE SALL
  initializeOnly SFNode ga USE GA
  initializeOnly SFNode goff USE GOFF
  initializeOnly SFNode tb USE TB
  initializeOnly SFNode tab USE TAB""", extra_functions="""
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'e') sa .enabled = false;
   if (k == 'f') sa .enabled = true;
   if (k == 'o') sall .objectType = new MFString ('NONE');
   if (k == 'p') sall .objectType = new MFString ('ALL');
   if (k == 't') sall .pickTarget = new MFNode (tab);
   if (k == 'g') ga .objectType = new MFString ('B');
   if (k == 'q') goff .pickable = true;
   if (k == 'm') tb .translation = new SFVec3f (-1.5, 5, 0);
}""") + "ROUTE K.keyPress TO R.on_key\n"
worlds["picking-groups.wrl"] = groups
keys = [(130, "e"), (135, "o"), (140, "t"), (145, "g"), (150, "q"), (155, "m"), (190, "f"), (195, "p")]
scripts["picking-groups.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 250, "capture": [30, 90, 170, 240], "dump": list(range(10, 250, 8)), "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

# --- picking-layers: each layer is picked on its own (a layer with pickable FALSE is skipped, until
# it is made pickable), an Inline as pick target, and a pick sensor inside an Inline (EXPORT/IMPORT,
# moved from the world) that is unloaded and loaded.
worlds["picking-inline-target.wrl"] = """#X3D V4.0 utf8
PROFILE Interchange
Transform { translation 2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.6 0.2 } } geometry Box { size 1.5 1.5 1.5 } } }
"""
worlds["picking-inline-sensor.wrl"] = HEAD + """DEF TT Transform { translation -1 -2 0 children """ + shape("Sphere { radius 0.8 }", "0.3 0.9 0.9") + """ }
DEF M Transform { translation -6 -2 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 0 } } geometry DEF L """ + LINE + """ }
  DEF IN LinePickSensor { intersectionType "GEOMETRY" pickingGeometry USE L pickTarget USE TT }
] }
EXPORT IN
EXPORT M
"""
layers = HEAD + """COMPONENT Layering:1
Viewpoint { position 0 0 14 }
DEF T0 Transform { translation 0 3 0 children """ + shape("Box { size 2 2 2 }", "0.9 0.3 0.2") + """ }
DEF INL Inline { url "picking-inline-target.wrl" }
DEF ISENS Inline { url "picking-inline-sensor.wrl" }
IMPORT ISENS.IN AS IN
IMPORT ISENS.M AS IM
DEF M0 Transform { translation -7 0 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF L0 """ + LINE + """ }
  DEF S0 LinePickSensor { sortOrder "ALL_SORTED" pickingGeometry USE L0 pickTarget [ USE T0 USE INL ] }
] }
DEF LS LayerSet { order [ 0 1 2 ] layers [
  DEF L1 Layer { children [
    Viewpoint { position 0 0 14 }
    DEF T1 Transform { translation 0 -5 0 children """ + shape("Box { size 2 2 2 }", "0.2 0.8 0.3") + """ }
    DEF M1 Transform { translation -7 -5 0 children [
      Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF L1G """ + LINE + """ }
      DEF S1 LinePickSensor { pickingGeometry USE L1G pickTarget USE T1 }
    ] }
  ] }
  DEF L2 Layer { pickable FALSE objectType [ "NONE" ] children [
    Viewpoint { position 0 0 14 }
    DEF T2 Transform { translation 0 5 0 children """ + shape("Box { size 2 2 2 }", "0.2 0.3 0.9") + """ }
    DEF M2 Transform { translation -7 5 0 children [
      Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF L2G """ + LINE + """ }
      DEF S2 LinePickSensor { pickingGeometry USE L2G pickTarget USE T2 }
    ] }
  ] }
] }
DEF C TimeSensor { cycleInterval 4 loop TRUE }
DEF P PositionInterpolator { key [ 0 1 ] keyValue [ -7 0 0, 7 0 0 ] }
ROUTE C.fraction_changed TO P.set_fraction
ROUTE P.value_changed TO M0.set_translation
DEF P1 PositionInterpolator { key [ 0 1 ] keyValue [ -7 -5 0, 7 -5 0 ] }
ROUTE C.fraction_changed TO P1.set_fraction
ROUTE P1.value_changed TO M1.set_translation
DEF P2 PositionInterpolator { key [ 0 1 ] keyValue [ -7 5 0, 7 5 0 ] }
ROUTE C.fraction_changed TO P2.set_fraction
ROUTE P2.value_changed TO M2.set_translation
DEF PI PositionInterpolator { key [ 0 1 ] keyValue [ -6 -2 0, 6 -2 0 ] }
ROUTE C.fraction_changed TO PI.set_fraction
ROUTE PI.value_changed TO IM.set_translation
DEF K KeySensor { }
""" + recorder([("S0", False), ("S1", False), ("S2", False), ("IN", True)], extra_fields="""
  inputOnly SFString on_key
  initializeOnly SFNode l2 USE L2
  initializeOnly SFNode l1 USE L1
  initializeOnly SFNode inl USE INL
  initializeOnly SFNode isens USE ISENS""", extra_functions="""
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'l') l2 .pickable = true;
   if (k == 'n') l1 .objectType = new MFString ('NONE');
   if (k == 'a') l1 .objectType = new MFString ('A');
   if (k == 'b') l1 .objectType = new MFString ('ALL');
   if (k == 'u') inl .load = false;
   if (k == 'v') inl .load = true;
   if (k == 's') isens .load = false;
   if (k == 'r') isens .load = true;
}""") + "ROUTE K.keyPress TO R.on_key\n"
worlds["picking-layers.wrl"] = layers
# The Inline reloads happen while no sensor is near a target: Rust runs a reloaded Inline's content
# one frame behind X_ITE (see REPORT), which these cases do not test.
# L2's objectType "NONE" is its initial value, which X_ITE does not forward to the layer's
# PickableGroup; the changes of L1's objectType are.
keys = [(125, "l"), (130, "u"), (135, "s"), (241, "v"), (242, "r"), (285, "n"), (330, "a"), (450, "b")]
scripts["picking-layers.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 480, "capture": [40, 100, 170, 300], "dump": list(range(10, 480, 10)), "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

out = sys.argv[1]
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
