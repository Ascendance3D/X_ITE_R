#!/usr/bin/env python3
"""REA batch 03: PointPickSensor, PrimitivePickSensor and VolumePickSensor fixtures.

Writes stepped worlds (with x10 scripts) into oracle/synthetic-rea-b03. The pick sensors' outputs
are routed to a Script whose inputOutput fields carry them into the scene-text dumps. The targets
cover every Ammo shape X_ITE builds (box, sphere, cone, cylinder, convex hull, triangle mesh,
height field, 2D hull, no shape) inside rotated and scaled Transforms.

    python3 -I oracle/tools/gen_rea_b03_picking.py oracle/synthetic-rea-b03
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

FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]


def recorder(sensors, extra_fields="", extra_functions=""):
    """A Script with, per sensor, the last value of each output and an event count."""
    fields, functions, routes = [], [], []
    for name, points in sensors:
        outs = [("isActive", "SFBool", "a"), ("pickedGeometry", "MFNode", "g")]
        if points:
            outs.append(("pickedPoint", "MFVec3f", "p"))
        for out, ty, short in outs:
            key = f"{name}_{short}"
            fields.append(f"  inputOnly {ty} on_{key}")
            fields.append(f"  inputOutput {ty} {key} {'FALSE' if ty == 'SFBool' else '[ ]'}")
            fields.append(f"  inputOutput SFInt32 {key}_count 0")
            log = f" log = log + '{key}' + (value .length === undefined ? (value ? 1 : 0) : value .length) + ' ';"
            functions.append(f"function on_{key} (value) {{ {key} = value; {key}_count = {key}_count + 1;{log} }}")
            routes.append(f"ROUTE {name}.{out} TO R.on_{key}")
    fields.append("  inputOutput SFString log \"\"")
    return ("DEF R Script {\n  directOutput TRUE\n" + "\n".join(fields) + extra_fields + '\n  url "javascript:\n' + "\n".join(functions) + extra_functions + '\n"\n}\n' + "\n".join(routes) + "\n")


def shape(geometry, color, transparency=0.3):
    return f"Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} transparency {transparency} }} }} geometry {geometry} }}"


OCTA = """IndexedFaceSet { coord Coordinate { point [ 1.2 0 0, -1.2 0 0, 0 1.2 0, 0 -1.2 0, 0 0 1.2, 0 0 -1.2 ] }
  coordIndex [ 0 2 4 -1 2 1 4 -1 1 3 4 -1 3 0 4 -1 2 0 5 -1 1 2 5 -1 3 1 5 -1 0 3 5 -1 ] }"""
GRID = """ElevationGrid { xDimension 4 zDimension 4 xSpacing 0.7 zSpacing 0.7 height [ 0 0.4 0.1 0, 0.3 1.2 0.8 0.2, 0 0.9 0.6 0.1, 0 0.2 0 0 ] solid FALSE }"""

# Row of targets on y = 0, one per Ammo shape kind.
TARGETS = [
    ("TBOX", "-9 0 0", "0 0 1 0.3", "1 1 1", "Box { size 2 2 2 }", "0.9 0.3 0.2"),
    ("TSPH", "-6 0 0", "0 0 1 0", "1 1 1", "Sphere { radius 1 }", "0.2 0.8 0.3"),
    ("TCON", "-3 0 0", "1 0 0 0.4", "1 1 1", "Cone { bottomRadius 1 height 2 }", "0.2 0.3 0.9"),
    ("TCYL", "0 0 0", "0 0 1 0.6", "1 1 1", "Cylinder { radius 1 height 2 }", "0.8 0.8 0.2"),
    ("THUL", "3 0 0", "0 1 0 0.5", "1 1 1", OCTA, "0.8 0.2 0.8"),
    ("TPCY", "6 0 0", "0 0 1 0", "1 1 1", "Cylinder { radius 1 height 2 top FALSE solid FALSE }", "0.3 0.8 0.8"),
    ("TELE", "8 -0.6 -1", "0 0 1 0", "1 1 1", GRID, "0.6 0.6 0.6"),
    ("TREC", "11.5 0 0", "0 1 0 0.3", "1 1 1", "Rectangle2D { size 2 2 }", "0.9 0.6 0.2"),
    ("TLIN", "14 0 0", "0 0 1 0", "1 1 1", "LineSet { vertexCount 2 coord Coordinate { point [ -1 0 0, 1 0 0 ] } }", "1 1 1"),
]


def targets(rows=TARGETS):
    return "\n".join(f"DEF {n} Transform {{ translation {t} rotation {r} scale {s} children {shape(g, c)} }}" for n, t, r, s, g, c in rows) + "\n"


def uses(rows=TARGETS):
    return " ".join(f"USE {r[0]}" for r in rows)


worlds, scripts = {}, {}

# --- vpick-point: a rotated, non-uniformly scaled point cloud sweeps the row (each sort order and
# both intersection types). A point exactly in the plane of a flat mesh is an exact boundary: X_ITE
# itself switches between contact and none from frame to frame there (REPORT, boundary evidence),
# so it is not in this world.
POINT_SENSORS = [("PGS", "GEOMETRY", "ALL_SORTED"), ("PGC", "GEOMETRY", "CLOSEST"), ("PGN", "GEOMETRY", "ANY"), ("PGA", "GEOMETRY", "ALL"), ("PBA", "BOUNDS", "ALL"), ("PBC", "BOUNDS", "CLOSEST")]
# (-1.02 0.33 0) was (-1.02 0.3 0): that point passed 0.0002 above a height field triangle for one
# frame, where Ammo's answer depends on the picker's history (ammo-known.tsv AMMO_PICKER_HISTORY).
points = "0 0 0, 0 0.7 0.2, 0 -0.95 0, 0.97 0 0, -1.02 0.33 0, 0.5 0.5 0.5, -0.6 -0.6 -0.2, 0 1.1 0, 0.3 -0.2 0.9, 1.15 0.9 0"
point_world = HEAD + """Viewpoint { position 1 0 26 }
NavigationInfo { headlight TRUE }
""" + targets() + """""" + """DEF MOVER Transform { rotation 0 0 1 0.2 scale 1.3 0.9 1 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF PTS PointSet { coord Coordinate { point [ """ + points + """ ] } } }
""" + "\n".join(f"  DEF {n} PointPickSensor {{ intersectionType \"{i}\" sortOrder \"{o}\" pickingGeometry USE PTS pickTarget [ {uses()} ] }}" for n, i, o in POINT_SENSORS) + """
] }
DEF CLOCK TimeSensor { cycleInterval 6 loop TRUE }
DEF PATH PositionInterpolator { key [ 0 1 ] keyValue [ -12 0.05 0.1, 16 -0.25 -0.1 ] }
ROUTE CLOCK.fraction_changed TO PATH.set_fraction
ROUTE PATH.value_changed TO MOVER.set_translation
""" + recorder([(n, i == "GEOMETRY") for n, i, _ in POINT_SENSORS])
worlds["vpick-point.wrl"] = point_world
scripts["vpick-point.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 186, "capture": [30, 90, 150], "dump": list(range(8, 186, 3)), "events": []}

# --- vpick-primitive: one sensor per primitive picking geometry (rotating while it moves), against
# the row; the cone target is scaled non-uniformly over time (btConeShape keeps its scaled radius
# between calls).
PRIMS = [("QBOX", "Box { size 1.6 1 1.2 }"), ("QSPH", "Sphere { radius 0.7 }"), ("QCON", "Cone { bottomRadius 0.8 height 1.4 }"), ("QCYL", "Cylinder { radius 0.6 height 1.6 }")]
rows = [r if r[0] != "TCON" else ("TCON", "-3 0 0", "1 0 0 0.4", "1 1 1", r[4], r[5]) for r in TARGETS]
prim_world = HEAD + """Viewpoint { position 1 0 30 }
NavigationInfo { headlight TRUE }
""" + targets(rows) + """DEF CLOCK TimeSensor { cycleInterval 7 loop TRUE }
"""
for k, (n, g) in enumerate(PRIMS):
    y = 3.2 - k * 2.1
    prim_world += f"""DEF M{n} Transform {{ translation -12 {y} 0 children [
  DEF R{n} Transform {{ children [ Shape {{ appearance Appearance {{ material Material {{ emissiveColor 1 1 1 }} }} geometry DEF G{n} {g} }}
    DEF {n} PrimitivePickSensor {{ intersectionType "GEOMETRY" sortOrder "ALL" pickingGeometry USE G{n} pickTarget [ {uses()} ] }}
    DEF B{n} PrimitivePickSensor {{ sortOrder "ALL_SORTED" pickingGeometry USE G{n} pickTarget [ {uses()} ] }} ] }}
] }}
DEF P{n} PositionInterpolator {{ key [ 0 1 ] keyValue [ -12 {y + 3.2 - 0.2} 0, 16 {-(y - 3.2) * 0.1 - 0.2} 0 ] }}
DEF O{n} OrientationInterpolator {{ key [ 0 0.5 1 ] keyValue [ 1 1 0 0, 1 1 0 3.1, 0 1 1 6 ] }}
ROUTE CLOCK.fraction_changed TO P{n}.set_fraction
ROUTE P{n}.value_changed TO M{n}.set_translation
ROUTE CLOCK.fraction_changed TO O{n}.set_fraction
ROUTE O{n}.value_changed TO R{n}.set_rotation
"""
prim_world += """DEF CSCALE PositionInterpolator { key [ 0 0.3 0.6 1 ] keyValue [ 1 1 1, 1.6 1 0.7, 0.8 1.2 1.4, 1 1 1 ] }
ROUTE CLOCK.fraction_changed TO CSCALE.set_fraction
ROUTE CSCALE.value_changed TO TCON.set_scale
""" + recorder([(n, False) for n, _ in PRIMS] + [("B" + n, False) for n, _ in PRIMS])
worlds["vpick-primitive.wrl"] = prim_world
scripts["vpick-primitive.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 216, "capture": [40, 110, 180], "dump": list(range(8, 216, 3)), "events": []}

# --- vpick-volume: a convex IndexedFaceSet and a scaled Sphere as picking volumes; PickableGroup
# objectType filters and matchCriterion; runtime changes of pickingGeometry (to a Box, to a LineSet:
# no shape, back), enabled, pickTarget, objectType, and a target's geometry.
vol_world = HEAD + """PROTO Holder [ inputOutput SFVec3f at 0 0 0 inputOutput MFNode kids [ ] ] { Transform { translation IS at children IS kids } }
Viewpoint { position 0 0 20 }
DEF GA PickableGroup { objectType [ "A" ] children DEF VA Transform { translation -5 0 0 rotation 0 0 1 0.7 children """ + shape("Box { size 1.5 1.5 1.5 }", "0.9 0.3 0.2") + """ } }
DEF GB PickableGroup { objectType [ "B" ] children DEF VB Transform { translation -2 0 0 children """ + shape("Sphere { radius 0.9 }", "0.2 0.8 0.3") + """ } }
DEF GAB PickableGroup { objectType [ "A" "B" ] children DEF VAB Transform { translation 1 0 0 scale 1 1.5 1 children """ + shape("DEF TGEO " + OCTA, "0.2 0.3 0.9") + """ } }
DEF VC Transform { translation 4 0 0 children """ + shape("Cone { bottomRadius 1 height 2 bottom FALSE solid FALSE }", "0.8 0.8 0.2") + """ }
DEF HP Holder { at 7 0 0 kids [ """ + shape("Cylinder { radius 0.8 height 1.5 }", "0.3 0.8 0.8") + """ ] }
DEF MOVER Transform { translation -9 0 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 1 } } geometry DEF VOL IndexedFaceSet { coord Coordinate { point [ 0 0.9 0, -0.6 -0.5 0.5, 0.6 -0.5 0.5, 0 -0.5 -0.7 ] } coordIndex [ 0 1 2 -1 0 2 3 -1 0 3 1 -1 1 3 2 -1 ] } }
  DEF VALL VolumePickSensor { intersectionType "GEOMETRY" sortOrder "ALL" pickingGeometry USE VOL pickTarget [ USE GA USE GB USE GAB USE VC USE HP ] }
  DEF VSA VolumePickSensor { objectType "A" intersectionType "GEOMETRY" sortOrder "ALL_SORTED" pickingGeometry USE VOL pickTarget [ USE GA USE GB USE GAB USE VC USE HP ] }
  DEF VSAB VolumePickSensor { objectType [ "A" "B" ] matchCriterion "MATCH_EVERY" intersectionType "GEOMETRY" sortOrder "ALL" pickingGeometry USE VOL pickTarget [ USE GA USE GB USE GAB ] }
  DEF VBND VolumePickSensor { sortOrder "ALL" pickingGeometry USE VOL pickTarget [ USE GA USE GB USE GAB USE VC USE HP ] }
] }
DEF MS Transform { translation -9 -3 0 scale 1.4 0.6 1 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 0 } } geometry DEF SPH Sphere { radius 0.8 } }
  DEF VSPH VolumePickSensor { intersectionType "GEOMETRY" sortOrder "CLOSEST" pickingGeometry USE SPH pickTarget [ USE VA USE VB USE VAB USE VC USE HP ] }
] }
DEF CLOCK TimeSensor { cycleInterval 5 loop TRUE }
DEF PATH PositionInterpolator { key [ 0 1 ] keyValue [ -9 0.1 0, 10 -0.1 0.2 ] }
DEF PATH2 PositionInterpolator { key [ 0 1 ] keyValue [ -9 -2 0, 10 1.5 0 ] }
ROUTE CLOCK.fraction_changed TO PATH.set_fraction
ROUTE PATH.value_changed TO MOVER.set_translation
ROUTE CLOCK.fraction_changed TO PATH2.set_fraction
ROUTE PATH2.value_changed TO MS.set_translation
DEF K KeySensor { }
""" + recorder([(n, False) for n in ("VALL", "VSA", "VSAB", "VBND", "VSPH")], extra_fields="""
  inputOnly SFString on_key
  initializeOnly SFNode vall USE VALL
  initializeOnly SFNode vsa USE VSA
  initializeOnly SFNode vsph USE VSPH
  initializeOnly SFNode ga USE GA
  initializeOnly SFNode vc USE VC
  initializeOnly SFNode tgeo USE TGEO""", extra_functions="""
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'b') vall .pickingGeometry = Browser .currentScene .createNode ('Box');
   if (k == 'l') { var l = Browser .currentScene .createNode ('LineSet'); vall .pickingGeometry = l; }
   if (k == 'r') vall .pickingGeometry = vsa .pickingGeometry;
   if (k == 'e') vsph .enabled = false;
   if (k == 'f') vsph .enabled = true;
   if (k == 't') vall .pickTarget = new MFNode (vc);
   if (k == 'g') ga .objectType = new MFString ('B');
   if (k == 'c') tgeo .coord .point [0] = new SFVec3f (2.2, 0, 0);
}""") + "ROUTE K.keyPress TO R.on_key\n"
worlds["vpick-volume.wrl"] = vol_world
keys = [(40, "b"), (55, "l"), (70, "r"), (80, "e"), (95, "c"), (100, "g"), (115, "f"), (130, "t")]
scripts["vpick-volume.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 156, "capture": [30, 90, 150], "dump": list(range(8, 156, 3)), "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

# --- vpick-inline: a PointPickSensor and a VolumePickSensor inside an Inline with their target
# (EXPORT/IMPORT; the world moves the sensors and records them); the Inline is unloaded and loaded
# while nothing is near the target.
worlds["vpick-inline-sensor.wrl"] = HEAD + """DEF T Transform { children """ + shape("Sphere { radius 1 }", "0.3 0.9 0.9") + """ }
DEF M Transform { translation -6 0 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 0 } } geometry DEF P PointSet { coord Coordinate { point [ 0 0 0, 0.6 0.4 0 ] } } }
  DEF IP PointPickSensor { intersectionType "GEOMETRY" pickingGeometry USE P pickTarget USE T }
  DEF IV VolumePickSensor { intersectionType "GEOMETRY" pickingGeometry Cone { bottomRadius 0.4 height 0.8 } pickTarget USE T }
] }
EXPORT IP
EXPORT IV
EXPORT M
"""
worlds["vpick-inline.wrl"] = HEAD + """Viewpoint { position 0 0 14 }
DEF ISENS Inline { url "vpick-inline-sensor.wrl" }
IMPORT ISENS.IP AS IP
IMPORT ISENS.IV AS IV
IMPORT ISENS.M AS IM
DEF C TimeSensor { cycleInterval 4 loop TRUE }
DEF PI PositionInterpolator { key [ 0 1 ] keyValue [ -6 0 0, 6 0 0 ] }
ROUTE C.fraction_changed TO PI.set_fraction
ROUTE PI.value_changed TO IM.set_translation
DEF K KeySensor { }
""" + recorder([("IP", True), ("IV", False)], extra_fields="""
  inputOnly SFString on_key
  initializeOnly SFNode isens USE ISENS""", extra_functions="""
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 's') isens .load = false;
   if (k == 'r') isens .load = true;
}""") + "ROUTE K.keyPress TO R.on_key\n"
# The reload happens while the sensors are far from the target (Rust runs a reloaded Inline's
# content one frame behind X_ITE, known defect 1).
scripts["vpick-inline.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 260, "capture": [60, 200], "dump": list(range(8, 260, 4)), "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in [(118, "s"), (122, "r")]]}

# vpick-layers: batch 02's picking-layers.wrl (LayerSet; a Layer made pickable and one whose
# objectType changes; an Inline target and an Inline sensor, both unloaded and loaded again by keys)
# with the batch 03 sensors instead of LinePickSensor, and the same event script.
B02 = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b02")
LINES = 'IndexedLineSet { coord Coordinate { point [ -1 0 0, 1 0 0, 0 -1 0.2, 0 1 0.2 ] } coordIndex [ 0 1 -1 2 3 -1 ] }'
TETRA = 'IndexedFaceSet { coord Coordinate { point [ -0.8 -0.6 -0.5, 0.8 -0.6 -0.5, 0 0.8 -0.5, 0 0 0.7 ] } coordIndex [ 0 1 2 -1 0 1 3 -1 1 2 3 -1 2 0 3 -1 ] }'
layers = open(os.path.join(B02, "picking-layers.wrl")).read()
for old, new in [
    ('DEF INL Inline { url "picking-inline-target.wrl" }', 'DEF INL Inline { url "../synthetic-rea-b02/picking-inline-target.wrl" }'),
    ('DEF ISENS Inline { url "picking-inline-sensor.wrl" }', 'DEF ISENS Inline { url "vpick-layers-sensor.wrl" }'),
    ('DEF L0 ' + LINES, 'DEF L0 Sphere { radius 0.6 }'),
    ('DEF S0 LinePickSensor { sortOrder "ALL_SORTED"', 'DEF S0 PrimitivePickSensor { intersectionType "GEOMETRY" sortOrder "ALL_SORTED"'),
    ('DEF L1G ' + LINES, 'DEF L1G ' + TETRA),
    ('DEF S1 LinePickSensor {', 'DEF S1 VolumePickSensor { intersectionType "GEOMETRY"'),
    ('DEF L2G ' + LINES, 'DEF L2G PointSet { coord Coordinate { point [ -0.5 0 0, 0.5 0 0, 0 0.5 0 ] } }'),
    ('DEF S2 LinePickSensor {', 'DEF S2 PointPickSensor { intersectionType "GEOMETRY"'),
    ("""  inputOnly MFVec3f on_IN_n
  inputOutput MFVec3f IN_n [ ]
  inputOutput SFInt32 IN_n_count 0
  inputOnly MFVec3f on_IN_t
  inputOutput MFVec3f IN_t [ ]
  inputOutput SFInt32 IN_t_count 0
""", ""),
    ("""function on_IN_n (value) { IN_n = value; IN_n_count = IN_n_count + 1; }
function on_IN_t (value) { IN_t = value; IN_t_count = IN_t_count + 1; }
""", ""),
    ("""ROUTE IN.pickedNormal TO R.on_IN_n
ROUTE IN.pickedTextureCoordinate TO R.on_IN_t
""", ""),
]:
    assert layers.count(old) == 1, old
    layers = layers.replace(old, new)
worlds["vpick-layers.wrl"] = layers
worlds["vpick-layers-sensor.wrl"] = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Picking:3
DEF TT Transform { translation -1 -2 0 children Shape { appearance Appearance { material Material { diffuseColor 0.3 0.9 0.9 transparency 0.3 } } geometry Sphere { radius 0.8 } } }
DEF M Transform { translation -6 -2 0 children [
  Shape { appearance Appearance { material Material { emissiveColor 1 1 0 } } geometry DEF L PointSet { coord Coordinate { point [ -0.3 0 0, 0.3 0 0.1 ] } } }
  DEF IN PointPickSensor { intersectionType "GEOMETRY" pickingGeometry USE L pickTarget USE TT }
] }
EXPORT IN
EXPORT M
"""
scripts["vpick-layers.json"] = json.load(open(os.path.join(B02, "picking-layers.json")))

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
