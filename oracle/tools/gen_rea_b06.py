#!/usr/bin/env python3
"""REA batch 06 (RigidBodyPhysics, 14 nodes): synthetic worlds + driver scripts.

  python3 -I oracle/tools/gen_rea_b06.py

writes oracle/synthetic-rea-b06/NAME.x3dv, oracle/synthetic-rea-b06/scripts/NAME.json and
oracle/synthetic-rea-b06/reload/*.x3dv. The same files drive pinned X_ITE (oracle/x_ite-anim.mjs, through
oracle/tools/rea_b06.py baseline) and x3d-drive (rea_b06.py check).

Every script has dt = 1/60, warmup 8 (the real warm-up is the baseline's warmupFrames), "console": true,
and a "physics" key: { "nodes": { PATH: [field, ...] } }. PATH is a DEF name, or "Inline/Name" for a DEF
inside a DEF-named Inline. The result of both sides carries
  physics.dt      [ { frame, dt } ]   every RigidBodyCollection .getTimeStep () result, in call order
                                       (frame 0 = load and warm-up)
  physics.frames  { "0".."N": { PATH: { type, FIELD: value } | null } }   read after warm-up and after each frame
Field values: numbers as doubles, SFVec3f [x,y,z], SFRotation [x,y,z,angle], SFNode as its DEF name (or
"<Type>"), MF* as lists. Pseudo fields: "@bodyActive" (btRigidBody isActive ()), and on a CollisionSensor
"contacts" (list of { position, contactNormal, depth, geometry1, geometry2, body1, body2 }).

Worlds use a fixed floor (a fixed RigidBody with a Box collidable, top at y = 0.5). Runtime field updates
come from a Script driven by a TimeSensor: it counts frames from its first event (the first warm-up
frame), so "frame K" means the K-th scripted frame.
"""
import json, os

d = os.path.join(os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")), "synthetic-rea-b06")

HEAD = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT RigidBodyPhysics:2\n\n"
DT = 1 / 60

# fields read per node type (the "physics.nodes" of the script)
TRACK = {
    "RigidBody": ["position", "orientation", "linearVelocity", "angularVelocity", "@bodyActive"],
    "RigidBodyCollection": ["enabled"],
    "CollidableShape": ["translation", "rotation"],
    "CollidableOffset": ["translation", "rotation"],
    "CollisionSensor": ["isActive", "intersections", "contacts"],
    "BallJoint": ["body1AnchorPoint", "body2AnchorPoint"],
    "SingleAxisHingeJoint": ["body1AnchorPoint", "body2AnchorPoint", "angle", "angleRate"],
    "DoubleAxisHingeJoint": ["body1AnchorPoint", "body2AnchorPoint", "body1Axis", "body2Axis", "hinge1Angle", "hinge2Angle", "hinge1AngleRate", "hinge2AngleRate"],
    "SliderJoint": ["separation", "separationRate"],
    "UniversalJoint": ["body1AnchorPoint", "body2AnchorPoint", "body1Axis", "body2Axis"],
    "MotorJoint": ["motor1Angle", "motor2Angle", "motor3Angle", "motor1AngleRate", "motor2AngleRate", "motor3AngleRate"],
}

worlds = {}  # name -> dict(text, frames, dump, track{path: fields}, settle)


def shape(geom):
    return f"Shape {{ geometry {geom} }}"


def collidable(name, geom, extra=""):
    return f"DEF {name} CollidableShape {{ {extra} shape {shape(geom)} }}"


def body(name, geom, pos=(0, 3, 0), extra="", cextra=""):
    """A RigidBody `name` with one CollidableShape `name_C`."""
    return f"DEF {name} RigidBody {{ position {pos[0]} {pos[1]} {pos[2]} {extra} geometry {collidable(name + '_C', geom, cextra)} }}"


def floor(name="F", size="20 1 20", pos=(0, 0, 0), geom=None):
    return body(name, geom or f"Box {{ size {size} }}", pos, "fixed TRUE")


def add(name, text, frames=120, dump=None, track=None, settle=False, extra_script=None):
    dump = dump if dump is not None else sorted({1, frames // 4, frames // 2, frames})
    worlds[name] = dict(text=HEAD + text, frames=frames, dump=dump, track=track or {}, settle=settle)


def tr(**kw):
    """track spec: tr(RigidBody=['A','B'], ...) -> { path: fields }"""
    out = {}
    for t, names in kw.items():
        for n in names:
            out[n] = TRACK[t]
    return out


def schedule(events):
    """events: [(frame, out_name, type, js_expr)] -> (Script text, ROUTE text). Routes are added by the caller."""
    outs = {}
    for _, o, t, _e in events:
        outs[o] = t
    decl = "".join(f"  outputOnly {t} {o}\n" for o, t in outs.items())
    body_ = "".join(f"  if (f >= {fr} && !done[{i}]) {{ done[{i}] = true; {o} = {e}; }}\n" for i, (fr, o, t, e) in enumerate(events))
    return (f"DEF TS TimeSensor {{ loop TRUE cycleInterval 1 }}\nDEF SC Script {{\n  inputOnly SFTime tick\n{decl}"
            f"  initializeOnly SFFloat s0 -1\n  url \"ecmascript:\nvar done = [ ];\nfunction tick (v)\n{{\n  if (s0 < 0) s0 = v;\n  var f = Math .round ((v - s0) * 60);\n{body_}}}\n\"\n}}\n"
            "ROUTE TS.time TO SC.tick\n")


def routes(events, targets):
    return "".join(f"ROUTE SC.{o} TO {targets[o]}\n" for o in dict.fromkeys(o for _, o, _t, _e in events))


def rbc(bodies, joints="", collider="", extra=""):
    return f"DEF RBC RigidBodyCollection {{ {extra} {('collider ' + collider) if collider else ''} bodies [ {bodies} ] {('joints [ ' + joints + ' ]') if joints else ''} }}\n"


# --------------------------------------------------------------------------------------------------------
# A. dynamics without collisions
add("fall-sphere", rbc(body("B", "Sphere { radius 1 }", (0, 10, 0), "mass 1")), 120, track=tr(RigidBody=["B"], RigidBodyCollection=["RBC"], CollidableShape=["B_C"]))

add("fall-masses", rbc(
    body("A", "Sphere { radius 0.5 }", (-2, 10, 0), "mass 1") + "\n" + body("B", "Box { size 1 2 3 }", (0, 10, 0), "mass 10") + "\n" +
    body("C", "Cylinder { radius 0.5 height 2 }", (2, 10, 0), "mass 0.1") + "\n" + body("D", "Cone { bottomRadius 0.5 height 1 }", (4, 10, 0), "mass 3 position 4 10 0")),
    120, track=tr(RigidBody=["A", "B", "C", "D"]))

add("inertia-ignored", rbc(
    body("A", "Box { size 1 2 3 }", (-3, 0, 0), "mass 2 inertia 1 0 0 0 1 0 0 0 1 torques [ 0 0 4, 1 0 0 ]") + "\n" +
    body("B", "Box { size 1 2 3 }", (3, 0, 0), "mass 2 inertia 50 0 0 0 7 0 0 0 0.5 torques [ 0 0 4, 1 0 0 ]") + "\n" +
    body("C", "Box { size 1 2 3 }", (0, 5, 0), "mass 2 inertia 9 1 2 3 4 5 6 7 8 torques [ 0 0 4, 1 0 0 ]"), extra="gravity 0 0 0"),
    120, track=tr(RigidBody=["A", "B", "C"]))

add("forces-torques", rbc(
    body("A", "Box { size 1 1 1 }", (-3, 0, 0), "mass 2 forces [ 1 0 0, 0 2 0 ] torques [ 0 0 1, 0 0.5 0 ]") + "\n" +
    body("B", "Sphere { radius 0.5 }", (0, 0, 0), "mass 1 torques [ 0.25 0.5 0.75 ]") + "\n" +
    body("C", "Cylinder { radius 0.5 height 1 }", (3, 0, 0), "mass 0.5 forces [ 0 0 -1 ] linearVelocity 0 1 0 angularVelocity 1 0 0"), extra="gravity 0 0 0"),
    120, track=tr(RigidBody=["A", "B", "C"]))

add("velocities", rbc(
    body("A", "Box { size 1 2 3 }", (0, 0, 0), "mass 1 linearVelocity 1 2 3 angularVelocity 0 1 0") + "\n" +
    body("B", "Sphere { radius 1 }", (5, 0, 0), "mass 1 linearVelocity -1 0 0.5 angularVelocity 1 2 3") + "\n" +
    body("C", "Cylinder { radius 0.5 height 2 }", (-5, 0, 0), "mass 1 angularVelocity 0.1 0 2.5"), extra="gravity 0 0 0"),
    120, track=tr(RigidBody=["A", "B", "C"]))

add("damping", rbc(
    body("A", "Box { size 1 1 1 }", (-3, 0, 0), "mass 1 linearVelocity 3 0 0 angularVelocity 0 0 3 autoDamp FALSE linearDampingFactor 0.5 angularDampingFactor 0.5") + "\n" +
    body("B", "Box { size 1 1 1 }", (0, 0, 0), "mass 1 linearVelocity 3 0 0 angularVelocity 0 0 3 autoDamp TRUE linearDampingFactor 0.5 angularDampingFactor 0.25") + "\n" +
    body("C", "Box { size 1 1 1 }", (3, 0, 0), "mass 1 linearVelocity 3 0 0 angularVelocity 0 0 3 autoDamp TRUE"), extra="gravity 0 0 0"),
    120, track=tr(RigidBody=["A", "B", "C"]))

add("gravity-flags", rbc(
    body("A", "Sphere { radius 0.5 }", (-3, 10, 0), "mass 1 useGlobalGravity TRUE") + "\n" +
    body("B", "Sphere { radius 0.5 }", (0, 10, 0), "mass 1 useGlobalGravity FALSE linearVelocity 1 0 0") + "\n" +
    body("C", "Sphere { radius 0.5 }", (3, 10, 0), "mass 1 fixed TRUE linearVelocity 5 5 5 angularVelocity 1 1 1") + "\n" +
    body("D", "Sphere { radius 0.5 }", (6, 10, 0), "mass 1 useGlobalGravity FALSE forces [ 0 -2 0 ]"), extra="gravity 0 -3 0"),
    90, track=tr(RigidBody=["A", "B", "C", "D"]))

add("finite-rotation", rbc(
    body("A", "Box { size 1 2 3 }", (-3, 0, 0), "mass 1 angularVelocity 1 1 1 useFiniteRotation TRUE finiteRotationAxis 0 1 0") + "\n" +
    body("B", "Box { size 1 2 3 }", (3, 0, 0), "mass 1 angularVelocity 1 1 1 useFiniteRotation FALSE finiteRotationAxis 0 1 0"), extra="gravity 0 0 0"),
    90, track=tr(RigidBody=["A", "B"]))

add("center-of-mass", rbc(
    body("A", "Box { size 1 1 1 }", (-3, 10, 0), "mass 1 centerOfMass 1 0 0 angularVelocity 0 0 1") + "\n" +
    body("B", "Box { size 1 1 1 }", (3, 10, 0), "mass 1 angularVelocity 0 0 1")),
    90, track=tr(RigidBody=["A", "B"]))

add("prefer-accuracy", (
    "DEF R1 RigidBodyCollection { preferAccuracy TRUE iterations 4 bodies [ " + floor("F1") + " " + body("A", "Box { size 1 1 1 }", (0, 2, 0), "mass 1 angularVelocity 1 0 1") + " ] }\n"
    "DEF R2 RigidBodyCollection { preferAccuracy FALSE iterations 25 bodies [ " + floor("F2", pos=(30, 0, 0)) + " " + body("B", "Box { size 1 1 1 }", (30, 2, 0), "mass 1 angularVelocity 1 0 1") + " ] }\n"
    "DEF R3 RigidBodyCollection { preferAccuracy TRUE iterations 1 gravity 0 -2 1 bodies [ " + floor("F3", pos=(60, 0, 0)) + " " + body("C", "Sphere { radius 0.5 }", (60, 2, 0), "mass 1") + " ] }\n"),
    120, track=tr(RigidBody=["A", "B", "C"]))

# --------------------------------------------------------------------------------------------------------
# B. shapes dropped onto a fixed floor, with a CollisionSensor
def drop2(name, geom, pos=(0, 3, 0), extra="mass 1", floor_geom=None, frames=180, rot=None, floor_pos=(0, 0, 0), cc_extra=""):
    ex = extra + (f" orientation {rot}" if rot else "")
    text = (f"DEF RBC RigidBodyCollection {{ collider DEF CC CollisionCollection {{ bounce 0.3 minBounceSpeed 0 appliedParameters [ \"BOUNCE\" ] {cc_extra} "
            f"collidables [ DEF F_C CollidableShape {{ shape {shape(floor_geom or 'Box { size 20 1 20 }')} }} DEF B_C CollidableShape {{ shape {shape(geom)} }} ] }}\n"
            f"  bodies [ DEF F RigidBody {{ fixed TRUE position {floor_pos[0]} {floor_pos[1]} {floor_pos[2]} geometry USE F_C }} DEF B RigidBody {{ position {pos[0]} {pos[1]} {pos[2]} {ex} geometry USE B_C }} ] }}\n"
            "DEF S CollisionSensor { collider USE CC }\n")
    add(name, text, frames, track={**tr(RigidBody=["F", "B"], CollisionSensor=["S"], CollidableShape=["B_C"])})


drop2("shape-sphere", "Sphere { radius 0.5 }")
drop2("shape-box", "Box { size 1 1 1 }", rot="0 0 1 0.4")
drop2("shape-cylinder", "Cylinder { radius 0.5 height 1.5 }", rot="1 0 0 0.3")
drop2("shape-cone", "Cone { bottomRadius 0.6 height 1.2 }", rot="1 0 1 0.5")
drop2("shape-cone-open", "Cone { bottomRadius 0.6 height 1.2 bottom FALSE }", extra="mass 1", floor_geom="Box { size 20 1 20 }")
drop2("shape-cylinder-open", "Cylinder { radius 0.5 height 1.5 top FALSE }")
drop2("shape-ifs-dynamic", "IndexedFaceSet { coord Coordinate { point [ -0.5 -0.5 -0.5, 0.5 -0.5 -0.5, 0.5 -0.5 0.5, -0.5 -0.5 0.5, 0 0.7 0 ] } coordIndex [ 0 1 2 3 -1 0 4 1 -1 1 4 2 -1 2 4 3 -1 3 4 0 -1 ] solid FALSE }")
drop2("shape-ifs-floor", "Sphere { radius 0.5 }", pos=(0.3, 3, 0.2),
      floor_geom="IndexedFaceSet { coord Coordinate { point [ -5 0 -5, 5 0 -5, 5 0 5, -5 0 5, 0 1 0 ] } coordIndex [ 0 1 4 -1 1 2 4 -1 2 3 4 -1 3 0 4 -1 ] solid FALSE }", frames=180)
drop2("shape-elevation", "Sphere { radius 0.5 }", pos=(2, 4, 3),
      floor_geom="ElevationGrid { xDimension 5 zDimension 5 xSpacing 2 zSpacing 2 height [ 0 0 0 0 0, 0 1 1 1 0, 0 1 2 1 0, 0 1 1 1 0, 0 0 0 0 0 ] solid FALSE }", floor_pos=(-4, 0, -4))
drop2("shape-elevation-origin", "Sphere { radius 0.5 }", pos=(3, 4, 5),
      floor_geom="ElevationGrid { xDimension 5 zDimension 5 xSpacing 2 zSpacing 2 height [ 0 0 0 0 0, 0 1 1 1 0, 0 1 2 1 0, 0 1 1 1 0, 0 0 0 0 0 ] solid FALSE }")
drop2("shape-box-box", "Box { size 2 1 1 }", pos=(0.3, 2, 0.1), rot="0 1 0 0.7")
drop2("shape-thickness", "Box { size 1 1 1 }", extra="mass 1")
worlds["shape-thickness"]["text"] = worlds["shape-thickness"]["text"].replace("DEF RBC RigidBodyCollection {", "DEF RBC RigidBodyCollection { contactSurfaceThickness 0.2")

# --------------------------------------------------------------------------------------------------------
# C. collision collection, offset, space
def cc_pair(i, bounce, params, minb, ballpos=(0, 4, 0), cc_extra=""):
    x = i * 30
    return (f"DEF R{i} RigidBodyCollection {{ collider DEF CC{i} CollisionCollection {{ bounce {bounce} minBounceSpeed {minb} appliedParameters [ {params} ] {cc_extra} collidables [ USE F{i}_C USE B{i}_C ] }}\n"
            f"  bodies [ DEF F{i} RigidBody {{ fixed TRUE position {x} 0 0 geometry DEF F{i}_C CollidableShape {{ shape Shape {{ geometry Box {{ size 10 1 10 }} }} }} }}\n"
            f"           DEF B{i} RigidBody {{ mass 1 position {x + ballpos[0]} {ballpos[1]} {ballpos[2]} geometry DEF B{i}_C CollidableShape {{ shape Shape {{ geometry Sphere {{ radius 0.5 }} }} }} }} ] }}\n")


add("collider-bounce",
    cc_pair(0, 0.8, '"BOUNCE"', 0) + cc_pair(1, 0.8, "", 0) + cc_pair(2, 0.8, '"BOUNCE"', 100) + cc_pair(3, 0.0, '"BOUNCE"', 0) + cc_pair(4, 1.0, '"BOUNCE"', 0) +
    cc_pair(5, 0.5, '"BOUNCE"', 0, cc_extra="enabled FALSE") + cc_pair(6, 0.8, '"BOUNCE"', 0.1) + cc_pair(7, 0.8, '"BOUNCE"', 3),
    240, track=tr(RigidBody=["B0", "B1", "B2", "B3", "B4", "B5", "B6", "B7"]))


def slide(i, fc, params, shape_="Box { size 1 1 1 }", av="0 0 0", ang_extra=""):
    x = i * 30
    return (f"DEF R{i} RigidBodyCollection {{ collider DEF CC{i} CollisionCollection {{ appliedParameters [ {params} ] frictionCoefficients {fc} collidables [ USE F{i}_C USE B{i}_C ] }}\n"
            f"  bodies [ DEF F{i} RigidBody {{ fixed TRUE position {x} 0 0 geometry DEF F{i}_C CollidableShape {{ shape Shape {{ geometry Box {{ size 60 1 10 }} }} }} }}\n"
            f"           DEF B{i} RigidBody {{ mass 1 position {x} 1.02 0 linearVelocity 6 0 0 angularVelocity {av} geometry DEF B{i}_C CollidableShape {{ shape Shape {{ geometry {shape_} }} }} }} ] }}\n")


add("collider-friction",
    slide(0, "0 0", '"FRICTION_COEFFICIENT_2"') + slide(1, "0.5 0", '"FRICTION_COEFFICIENT_2"') + slide(2, "1.0 0", '"FRICTION_COEFFICIENT_2"') +
    slide(3, "0.5 0", '"BOUNCE"') + slide(4, "0.5 0.5", '"FRICTION_COEFFICIENT_2"', "Sphere { radius 0.5 }", "0 0 -12") + slide(5, "0.5 0", '"FRICTION_COEFFICIENT_2"', "Sphere { radius 0.5 }", "0 0 -12"),
    180, track=tr(RigidBody=["B0", "B1", "B2", "B3", "B4", "B5"]))

add("collidable-offset", (
    "DEF RBC RigidBodyCollection { bodies [ DEF F RigidBody { fixed TRUE geometry DEF F_C CollidableShape { shape Shape { geometry Box { size 20 1 20 } } } }\n"
    "  DEF A RigidBody { mass 1 position -4 3 0 geometry DEF A_O CollidableOffset { translation 0.5 0 0 rotation 0 0 1 0.3 collidable DEF A_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } } }\n"
    "  DEF B RigidBody { mass 1 position 0 3 0 geometry DEF B_O CollidableOffset { translation 1 0 0 collidable DEF B_O2 CollidableOffset { translation 0 0.5 0 rotation 0 1 0 0.5 collidable DEF B_C CollidableShape { shape Shape { geometry Sphere { radius 0.4 } } } } } }\n"
    "  DEF C RigidBody { mass 1 position 4 3 0 geometry [ DEF C_C CollidableShape { translation -0.5 0 0 shape Shape { geometry Box { size 1 1 1 } } } DEF C_D CollidableShape { translation 0.5 0 0 rotation 0 0 1 0.5 shape Shape { geometry Box { size 1 1 1 } } } ] } ] }\n"
    "DEF CC CollisionCollection { collidables [ USE F_C USE A_O USE B_O USE C_C USE C_D ] }\n"
    "DEF S CollisionSensor { collider USE CC }\n"),
    180, track={**tr(RigidBody=["A", "B", "C"], CollisionSensor=["S"]), "A_O": TRACK["CollidableOffset"], "B_O": TRACK["CollidableOffset"], "B_O2": TRACK["CollidableOffset"], "C_C": TRACK["CollidableShape"], "C_D": TRACK["CollidableShape"]})

add("collision-space", (
    "DEF RBC RigidBodyCollection { bodies [ DEF F RigidBody { fixed TRUE geometry DEF F_C CollidableShape { shape Shape { geometry Box { size 20 1 20 } } } }\n"
    "  DEF A RigidBody { mass 1 position -1 3 0 geometry DEF A_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } }\n"
    "  DEF B RigidBody { mass 1 position 1 3 0 geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }\n"
    "  DEF D RigidBody { mass 1 position 3 3 0 geometry DEF D_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } } ] }\n"
    "DEF SP CollisionSpace { useGeometry TRUE collidables [ USE A_C USE B_C ] }\n"
    "DEF SP2 CollisionSpace { collidables [ USE F_C USE D_C ] }\n"
    "DEF CC CollisionCollection { collidables [ USE SP USE SP2 ] }\n"
    "DEF S CollisionSensor { collider USE CC }\n"
    "DEF CC2 CollisionCollection { collidables [ USE SP USE B_C ] }\n"
    "DEF S2 CollisionSensor { collider USE CC2 }\n"),
    150, track={**tr(RigidBody=["A", "B", "D"], CollisionSensor=["S", "S2"])})

add("sensor-partial", (
    "DEF RBC RigidBodyCollection { bodies [ DEF F RigidBody { fixed TRUE geometry DEF F_C CollidableShape { shape Shape { geometry Box { size 20 1 20 } } } }\n"
    "  DEF A RigidBody { mass 1 position 0 2 0 geometry DEF A_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } }\n"
    "  DEF B RigidBody { mass 1 position 0.2 4 0 geometry DEF B_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } } ] }\n"
    "DEF CC1 CollisionCollection { bounce 0.6 minBounceSpeed 0 collidables [ USE A_C ] }\n"
    "DEF S1 CollisionSensor { collider USE CC1 }\n"
    "DEF CC2 CollisionCollection { collidables [ USE F_C ] }\n"
    "DEF S2 CollisionSensor { collider USE CC2 }\n"
    "DEF S3 CollisionSensor { }\n"
    "DEF CC4 CollisionCollection { collidables [ USE A_C USE B_C USE F_C ] }\n"
    "DEF S4 CollisionSensor { collider USE CC4 enabled FALSE }\n"),
    180, track={**tr(RigidBody=["A", "B"], CollisionSensor=["S1", "S2", "S3", "S4"])})

# --------------------------------------------------------------------------------------------------------
# E. joints
FO = '[ "body1AnchorPoint" "body2AnchorPoint" ]'


def joint_world(name, bodies2, joint, frames=240, jtype="BallJoint", extra=""):
    text = (f"DEF RBC RigidBodyCollection {{ {extra} bodies [ DEF A RigidBody {{ fixed TRUE position 0 5 0 geometry DEF A_C CollidableShape {{ shape Shape {{ geometry Box {{ size 0.5 0.5 0.5 }} }} }} }}\n"
            f"  {bodies2} ]\n  joints [ {joint} ] }}\n")
    add(name, text, frames, track={**tr(RigidBody=["A", "B"]), "J": TRACK[jtype]})


B_SPH = "DEF B RigidBody { mass 1 position 2 5 0 geometry DEF B_C CollidableShape { shape Shape { geometry Sphere { radius 0.3 } } } }"
B_BOX = "DEF B RigidBody { mass 1 position 1.5 5 0 geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 3 0.4 0.4 } } } }"

joint_world("joint-ball", B_SPH, 'DEF J BallJoint { body1 USE A body2 USE B anchorPoint 0 5 0 forceOutput [ "ALL" ] }', 240)
joint_world("joint-ball-offaxis", B_SPH.replace("2 5 0", "1 5 1"), 'DEF J BallJoint { body1 USE A body2 USE B anchorPoint 0 5 0.5 forceOutput [ "body1AnchorPoint" ] }', 240)
joint_world("joint-hinge-single", B_BOX, 'DEF J SingleAxisHingeJoint { body1 USE A body2 USE B anchorPoint 0 5 0 axis 0 0 1 minAngle -1 maxAngle 0.5 forceOutput [ "body1AnchorPoint" "body2AnchorPoint" "angle" "angleRate" ] }', 240, "SingleAxisHingeJoint")
joint_world("joint-hinge-single-all", B_BOX, 'DEF J SingleAxisHingeJoint { body1 USE A body2 USE B anchorPoint 0 5 0 axis 0 0 1 forceOutput [ "ALL" ] }', 240, "SingleAxisHingeJoint")
joint_world("joint-hinge-double", B_BOX, 'DEF J DoubleAxisHingeJoint { body1 USE A body2 USE B anchorPoint 0 5 0 axis1 0 0 1 axis2 0 1 0 desiredAngularVelocity1 2 maxTorque1 50 suspensionForce 1 forceOutput [ "ALL" ] }', 240, "DoubleAxisHingeJoint")
joint_world("joint-hinge-double-rate", B_BOX, 'DEF J DoubleAxisHingeJoint { body1 USE A body2 USE B anchorPoint 0 5 0 axis1 0 0 1 axis2 0 1 0 forceOutput [ "body1Axis" "body2Axis" "hinge1Angle" "hinge2Angle" "angleRate" ] }', 240, "DoubleAxisHingeJoint")
joint_world("joint-slider", "DEF B RigidBody { mass 1 position 1 5 0 forces [ 5 0 0 ] geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }",
            'DEF J SliderJoint { body1 USE A body2 USE B axis 1 0 0 minSeparation 0 maxSeparation 2 sliderForce 3 forceOutput [ "ALL" ] }', 240, "SliderJoint", extra="gravity 0 0 0")
joint_world("joint-slider-gravity", "DEF B RigidBody { mass 1 position 0 4 0 geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }",
            'DEF J SliderJoint { body1 USE A body2 USE B axis 0 1 0 minSeparation -1 maxSeparation 1 }', 240, "SliderJoint")
joint_world("joint-universal", B_SPH, 'DEF J UniversalJoint { body1 USE A body2 USE B anchorPoint 0 5 0 axis1 1 0 0 axis2 0 0 1 forceOutput [ "ALL" ] }', 120, "UniversalJoint")
joint_world("joint-motor", B_SPH, 'DEF J MotorJoint { body1 USE A body2 USE B autoCalc TRUE enabledAxes 3 axis1Torque 1 axis2Torque 1 motor1Axis 1 0 0 forceOutput [ "ALL" ] }', 120, "MotorJoint")
add("joint-chain", (
    "DEF RBC RigidBodyCollection { bodies [ DEF A RigidBody { fixed TRUE position 0 8 0 geometry DEF A_C CollidableShape { shape Shape { geometry Box { size 0.5 0.5 0.5 } } } }\n"
    "  DEF B RigidBody { mass 1 position 1 8 0 geometry DEF B_C CollidableShape { shape Shape { geometry Sphere { radius 0.2 } } } }\n"
    "  DEF C RigidBody { mass 1 position 2 8 0 geometry DEF C_C CollidableShape { shape Shape { geometry Sphere { radius 0.2 } } } }\n"
    "  DEF D RigidBody { mass 1 position 3 8 0 geometry DEF D_C CollidableShape { shape Shape { geometry Sphere { radius 0.2 } } } } ]\n"
    "  joints [ DEF J1 BallJoint { body1 USE A body2 USE B anchorPoint 0 8 0 } DEF J2 BallJoint { body1 USE B body2 USE C anchorPoint 1 8 0 forceOutput [ \"ALL\" ] } DEF J3 BallJoint { body1 USE C body2 USE D anchorPoint 2 8 0 } ] }\n"),
    240, track={**tr(RigidBody=["B", "C", "D"]), "J2": TRACK["BallJoint"]})

# --------------------------------------------------------------------------------------------------------
# F. sleep / wake
SLEEP_BODY = ("DEF B RigidBody { mass 1 position 0 1.2 0 autoDisable TRUE disableTime 0.5 disableLinearSpeed 0.5 disableAngularSpeed 0.5 "
              "geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } }")
SLEEP_BODY2 = SLEEP_BODY.replace("DEF B ", "DEF B2 ").replace("B_C", "B2_C").replace("0 1.2 0", "3 1.2 0").replace("autoDisable TRUE", "autoDisable FALSE")
add("sleep-rest", (
    "DEF RBC RigidBodyCollection { autoDisable TRUE disableTime 0.5 disableLinearSpeed 0.5 disableAngularSpeed 0.5 bodies [ " + floor() + "\n  " + SLEEP_BODY + "\n  " + SLEEP_BODY2 + " ] }\n"),
    420, dump=[1, 100, 200, 300, 420], track=tr(RigidBody=["B", "B2"]))

ev = [(240, "lv", "SFVec3f", "new SFVec3f (2, 5, 0)")]
add("sleep-wake", (
    "DEF RBC RigidBodyCollection { bodies [ " + floor() + "\n  " + SLEEP_BODY + "\n  " + SLEEP_BODY2 + " ] }\n" + schedule(ev) + routes(ev, {"lv": "B.set_linearVelocity"})),
    420, dump=[1, 100, 239, 241, 300, 420], track=tr(RigidBody=["B", "B2"]))

# --------------------------------------------------------------------------------------------------------
# G. runtime field updates
def rt(name, text, ev, targets, frames, track, dump=None):
    add(name, text + schedule(ev) + routes(ev, targets), frames, dump=dump, track=track)


BX = lambda n, x, extra="mass 1": body(n, "Box { size 1 1 1 }", (x, 5, 0), extra)

ev = [(20, "lv", "SFVec3f", "new SFVec3f (3, 4, 0)"), (30, "av", "SFVec3f", "new SFVec3f (0, 0, 5)"), (40, "fo", "MFVec3f", "new MFVec3f (new SFVec3f (0, 40, 0))"),
      (60, "ms", "SFFloat", "10"), (70, "fx", "SFBool", "true"), (90, "en", "SFBool", "false"), (100, "en2", "SFBool", "true"), (110, "pos", "SFVec3f", "new SFVec3f (10, 9, 0)"),
      (130, "ori", "SFRotation", "new SFRotation (0, 0, 1, 1)"), (140, "tq", "MFVec3f", "new MFVec3f (new SFVec3f (0, 0, 3))"), (150, "ug", "SFBool", "false"),
      (160, "ad", "SFBool", "true"), (170, "ld", "SFFloat", "2")]
rt("rt-body", rbc(floor() + "\n" + BX("A", -6) + "\n" + BX("B", -3) + "\n" + BX("C", 0, "mass 1 fixed FALSE") + "\n" + BX("D", 3, "mass 1 enabled TRUE") + "\n" + BX("E", 6) + "\n" + BX("G", 9, "mass 1 autoDamp FALSE linearDampingFactor 0.5")),
   ev, {"lv": "A.set_linearVelocity", "av": "B.set_angularVelocity", "fo": "B.set_forces", "ms": "C.set_mass", "fx": "C.set_fixed", "en": "D.set_enabled", "en2": "D.set_enabled",
        "pos": "E.set_position", "ori": "E.set_orientation", "tq": "E.set_torques", "ug": "G.set_useGlobalGravity", "ad": "G.set_autoDamp", "ld": "G.set_linearDampingFactor"},
   240, tr(RigidBody=["A", "B", "C", "D", "E", "G"]), dump=[1, 20, 21, 40, 41, 60, 71, 90, 100, 111, 130, 150, 170, 240])

ev = [(20, "g", "SFVec3f", "new SFVec3f (0, 5, 0)"), (40, "en", "SFBool", "false"), (60, "en", "SFBool", "true"), (80, "ec", "SFFloat", "0.1"), (100, "ad", "SFBool", "true"),
      (120, "cs", "SFFloat", "0.5")]
rt("rt-collection", rbc(floor() + "\n" + BX("A", -3) + "\n" + BX("B", 3), extra="contactSurfaceThickness 0.0"), ev,
   {"g": "RBC.set_gravity", "en": "RBC.set_enabled", "ec": "RBC.set_errorCorrection", "ad": "RBC.set_autoDisable", "cs": "RBC.set_contactSurfaceThickness"},
   200, tr(RigidBody=["A", "B"], RigidBodyCollection=["RBC"]), dump=[1, 20, 21, 40, 50, 60, 80, 100, 120, 130, 200])

ev = [(20, "bn", "SFFloat", "0.9"), (40, "mb", "SFFloat", "100"), (60, "ap", "MFString", "new MFString ()"), (80, "ap", "MFString", "new MFString ('BOUNCE')"), (80, "bn", "SFFloat", "0.9"), (100, "ce", "SFBool", "false"), (120, "ce", "SFBool", "true"),
      (140, "fc", "SFVec2f", "new SFVec2f (0.1, 0)"), (140, "ap2", "MFString", "new MFString ('BOUNCE', 'FRICTION_COEFFICIENT_2')")]
RT_CC = {"bn": "CC.set_bounce", "mb": "CC.set_minBounceSpeed", "ap": "CC.set_appliedParameters", "ce": "CC.set_enabled", "fc": "CC.set_frictionCoefficients", "ap2": "CC.set_appliedParameters"}
rt("rt-collider",
   "DEF RBC RigidBodyCollection { collider DEF CC CollisionCollection { bounce 0.3 minBounceSpeed 0 collidables [ DEF F_C CollidableShape { shape Shape { geometry Box { size 20 1 20 } } } DEF A_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } DEF B_C CollidableShape { shape Shape { geometry Box { size 1 1 1 } } } ] }\n"
   "  bodies [ DEF F RigidBody { fixed TRUE geometry USE F_C } DEF A RigidBody { mass 1 position -3 4 0 geometry USE A_C } DEF B RigidBody { mass 1 position 3 4 0 linearVelocity 3 0 0 geometry USE B_C } ] }\n"
   "DEF S CollisionSensor { collider USE CC }\n", ev, RT_CC, 240, tr(RigidBody=["A", "B"], CollisionSensor=["S"]), dump=[1, 20, 60, 100, 140, 240])

ev = [(30, "an", "SFVec3f", "new SFVec3f (0, 5, 1)"), (60, "ax", "SFVec3f", "new SFVec3f (0, 1, 0)"), (90, "mn", "SFFloat", "-0.2"), (120, "fo", "MFString", "new MFString ('NONE')")]
rt("rt-joint", (
    "DEF RBC RigidBodyCollection { bodies [ DEF A RigidBody { fixed TRUE position 0 5 0 geometry DEF A_C CollidableShape { shape Shape { geometry Box { size 0.5 0.5 0.5 } } } }\n"
    "  DEF B RigidBody { mass 1 position 1.5 5 0 geometry DEF B_C CollidableShape { shape Shape { geometry Box { size 3 0.4 0.4 } } } }\n"
    "  DEF C RigidBody { mass 1 position 1 5 6 geometry DEF C_C CollidableShape { shape Shape { geometry Sphere { radius 0.3 } } } } ]\n"
    "  joints [ DEF J SingleAxisHingeJoint { body1 USE A body2 USE B anchorPoint 0 5 0 axis 0 0 1 forceOutput [ \"angle\" \"angleRate\" \"body2AnchorPoint\" ] }\n"
    "           DEF J2 BallJoint { body1 USE A body2 USE C anchorPoint 0 5 6 forceOutput [ \"ALL\" ] } ] }\n"),
   ev, {"an": "J2.set_anchorPoint", "ax": "J.set_axis", "mn": "J.set_minAngle", "fo": "J2.set_forceOutput"}, 200,
   {**tr(RigidBody=["B", "C"]), "J": TRACK["SingleAxisHingeJoint"], "J2": TRACK["BallJoint"]}, dump=[1, 29, 31, 61, 121, 200])

ev = [(40, "en", "SFBool", "false"), (80, "en", "SFBool", "true"), (110, "co", "SFNode", "null")]
rt("rt-sensor", (
    "DEF RBC RigidBodyCollection { bodies [ " + floor() + "\n" + body("A", "Sphere { radius 0.5 }", (0, 2, 0), "mass 1") + "\n" + body("B", "Sphere { radius 0.5 }", (3, 2, 0), "mass 1") + " ] }\n"
    "DEF CC CollisionCollection { bounce 0.8 minBounceSpeed 0 collidables [ USE F_C USE A_C USE B_C ] }\n"
    "DEF S CollisionSensor { collider USE CC }\n"),
   ev[:2], {"en": "S.set_enabled"}, 200, tr(RigidBody=["A", "B"], CollisionSensor=["S"]), dump=[1, 39, 41, 79, 81, 120, 200])

# reload: an Inline whose content is a physics world, unloaded at frame 40 and loaded again at frame 90
RELOAD_CONTENT = HEAD + (
    "DEF RBC RigidBodyCollection { collider DEF CC CollisionCollection { bounce 0.5 minBounceSpeed 0 collidables [ DEF F_C CollidableShape { shape Shape { geometry Box { size 20 1 20 } } } DEF B_C CollidableShape { shape Shape { geometry Sphere { radius 0.5 } } } ] }\n"
    "  bodies [ DEF F RigidBody { fixed TRUE geometry USE F_C } DEF B RigidBody { mass 1 position 0 4 0 geometry USE B_C } ] }\n"
    "DEF S CollisionSensor { collider USE CC }\nEXPORT B\nEXPORT S\n")
RELOAD = HEAD + (
    "DEF I Inline { url \"reload/reload-content.x3dv\" }\n"
    "DEF TS TimeSensor { loop TRUE cycleInterval 1 }\n"
    "DEF SC Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode inline USE I\n  directOutput TRUE\n  initializeOnly SFFloat s0 -1\n"
    "  url \"ecmascript:\nvar done = [ ];\nfunction tick (v)\n{\n  if (s0 < 0) s0 = v;\n  var f = Math .round ((v - s0) * 60);\n"
    "  if (f >= 40 && !done[0]) { done[0] = true; inline .load = false; }\n  if (f >= 90 && !done[1]) { done[1] = true; inline .load = true; }\n}\n\"\n}\n"
    "ROUTE TS.time TO SC.tick\n")
worlds["reload-inline"] = dict(text=RELOAD, frames=200, dump=[1, 39, 41, 89, 91, 92, 120, 200], settle=True,
                               track={"I/RBC": TRACK["RigidBodyCollection"], "I/B": TRACK["RigidBody"], "I/S": TRACK["CollisionSensor"]})

# fields: every node with non-default fields, scene text only
add("fields-roundtrip", (
    "DEF RBC RigidBodyCollection { enabled FALSE preferAccuracy TRUE errorCorrection 0.5 iterations 7 gravity 1 -2 3 constantForceMix 0.01 maxCorrectionSpeed 2 contactSurfaceThickness 0.05 autoDisable TRUE disableTime 3 disableLinearSpeed 0.1 disableAngularSpeed 0.2 visible FALSE bboxDisplay TRUE bboxSize 1 2 3 bboxCenter 4 5 6\n"
    "  collider DEF CC CollisionCollection { enabled FALSE appliedParameters [ \"BOUNCE\" \"FRICTION_COEFFICIENT_2\" \"SLIP_COEFFICIENT_2\" \"BOGUS\" ] bounce 0.7 minBounceSpeed 0.2 frictionCoefficients 0.3 0.4 surfaceSpeed 0.5 0.6 slipFactors 0.7 0.8 softnessConstantForceMix 0.02 softnessErrorCorrection 0.3 visible FALSE bboxDisplay TRUE bboxSize 1 1 1 bboxCenter 1 1 1 collidables [ USE A_O USE SP ] }\n"
    "  bodies [ DEF A RigidBody { fixed TRUE position 1 2 3 orientation 0 1 0 1 linearVelocity 1 2 3 angularVelocity 4 5 6 useFiniteRotation TRUE finiteRotationAxis 1 0 0 autoDamp TRUE linearDampingFactor 0.1 angularDampingFactor 0.2 mass 3 inertia 1 2 3 4 5 6 7 8 9 centerOfMass 1 1 1 forces [ 1 1 1 ] torques [ 2 2 2 ] useGlobalGravity FALSE autoDisable TRUE disableTime 4 disableLinearSpeed 0.3 disableAngularSpeed 0.4 enabled FALSE bboxDisplay TRUE bboxSize 2 2 2 bboxCenter 3 3 3\n"
    "    geometry DEF A_O CollidableOffset { enabled FALSE translation 1 2 3 rotation 0 0 1 2 visible FALSE bboxDisplay TRUE bboxSize 1 1 1 bboxCenter 2 2 2 collidable DEF A_C CollidableShape { enabled FALSE translation 3 2 1 rotation 1 0 0 1 visible FALSE bboxDisplay TRUE bboxSize 1 1 1 bboxCenter 1 2 3 shape Shape { geometry Box { } } } } } ]\n"
    "  joints [ DEF J1 BallJoint { forceOutput [ \"body1AnchorPoint\" ] anchorPoint 1 2 3 body1 USE A }\n"
    "           DEF J2 SingleAxisHingeJoint { forceOutput [ \"ALL\" ] anchorPoint 1 2 3 axis 1 1 0 minAngle -1 maxAngle 2 stopBounce 0.1 stopErrorCorrection 0.2 body2 USE A }\n"
    "           DEF J3 DoubleAxisHingeJoint { forceOutput [ \"ALL\" ] anchorPoint 1 2 3 axis1 0 0 1 axis2 1 0 0 minAngle1 -1 maxAngle1 1 desiredAngularVelocity1 1 desiredAngularVelocity2 2 maxTorque1 3 maxTorque2 4 stop1Bounce 0.1 stop1ConstantForceMix 0.2 stop1ErrorCorrection 0.3 suspensionForce 0.4 suspensionErrorCorrection 0.5 }\n"
    "           DEF J4 SliderJoint { forceOutput [ \"separation\" ] axis 1 0 0 minSeparation -1 maxSeparation 2 sliderForce 3 stopBounce 0.1 stopErrorCorrection 0.2 }\n"
    "           DEF J5 UniversalJoint { forceOutput [ \"ALL\" ] anchorPoint 1 2 3 axis1 0 1 0 axis2 0 0 1 stop1Bounce 0.1 stop2Bounce 0.2 stop1ErrorCorrection 0.3 stop2ErrorCorrection 0.4 }\n"
    "           DEF J6 MotorJoint { autoCalc TRUE forceOutput [ \"ALL\" ] axis1Angle 1 axis2Angle 2 axis3Angle 3 axis1Torque 4 axis2Torque 5 axis3Torque 6 enabledAxes 3 motor1Axis 0 1 0 motor2Axis 0 0 1 motor3Axis 1 0 0 stop1Bounce 0.1 stop2Bounce 0.2 stop3Bounce 0.3 stop1ErrorCorrection 0.4 stop2ErrorCorrection 0.5 stop3ErrorCorrection 0.6 } ] }\n"
    "DEF SP CollisionSpace { enabled FALSE useGeometry TRUE visible FALSE bboxDisplay TRUE bboxSize 1 2 3 bboxCenter 4 5 6 collidables [ USE A_C ] }\n"
    "DEF S CollisionSensor { enabled FALSE description \"d\" collider USE CC }\n"),
    2, dump=[1, 2], track={"A": TRACK["RigidBody"], "S": TRACK["CollisionSensor"], "J6": TRACK["MotorJoint"]})


def write(name, w):
    spec = {p: f for p, f in w["track"].items()}
    script = {"dt": DT, "frames": w["frames"], "warmup": 8, "dump": w["dump"], "console": True, "physics": {"nodes": spec}}
    if w["settle"]:
        script["settle"] = True
        script["dumpInlines"] = True
    ext = "x3dv"
    open(os.path.join(d, name + "." + ext), "w").write(w["text"])
    json.dump(script, open(os.path.join(d, "scripts", name + ".json"), "w"))


if __name__ == "__main__":
    os.makedirs(os.path.join(d, "scripts"), exist_ok=True)
    os.makedirs(os.path.join(d, "reload"), exist_ok=True)
    for name, w in sorted(worlds.items()):
        write(name, w)
    # the Inline content of reload-inline
    open(os.path.join(d, "reload", "reload-content.x3dv"), "w").write(RELOAD_CONTENT)
    print(f"{len(worlds)} worlds in {d}")
