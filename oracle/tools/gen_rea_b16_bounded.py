#!/usr/bin/env python3
"""BoundedPhysicsModel replay (lane XITE_R_BOUNDED_PHYSICS_REPLAY_01): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b16_bounded.py

writes oracle/synthetic-rea-b16-bounded/NAME.x3dv and scripts/NAME.json for `x3d-oracle particles` and the
particles_b06_common replay (crates/x3d-viewer/tests/rea_b16_bounded_physics.rs). Every case dumps the whole
transform feedback buffer of PS every frame. Particles fall from (0.3 2 -0.2) along -Y onto a floor at y = 0 unless
the case says otherwise; the emitters have no speed, lifetime or angle variation, so the seeds move no particle.

  bp-hits-N        N hits on the bounce line: the floor and N - 1 planes above the emitter (behind every falling
                   particle). The GLSL intersection arrays hold ARRAY_SIZE = 32 entries; getIntersections does not
                   check the bound.
  bp-hits-N-below  the floor and N - 1 planes below it (ahead of the falling particle, farther than the floor).
  bp-hits-64-floor-last, bp-hits-64-mixed: the floor listed after 63 planes below it, and between 32 planes above
                   and 31 below.
  bp-damping-*     damping 0: the normals are multiplied by 0 before the BVH is built (normalize of a zero vector).
  bp-tilted, bp-transformed, bp-normals*: the floor turned in its own coordinates, the whole ParticleSystem under a
                   Transform, and per-vertex normals (interpolated, then normalized; their length is the damping).
  bp-enabled-*, bp-damping-change: field events on the BoundedPhysicsModel BM.
  bp-rebuild, bp-replace: a Script changes the floor's Coordinate (geometry rebuild) or replaces BM.geometry,
                   run by a field event (SW.go) before frame 21.
  bp-ts-*, bp-tick-*: the same changes at a time the Script chooses. bp-ts-*: a TimeSensor with a startTime between
                   frames 20 and 21 (a virtual timer of the oracle page) routes isActive to SW.go. bp-tick-*: a looping
                   TimeSensor routes time to SW.tick every frame; the Script decides from the tick (once, at the first
                   tick from 20.5 frames on; or on every 12th tick; or with a node it builds from a string).

Newly authored; no historical content.
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b16-bounded"))
HEAD = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n\n"
T0 = 946684800
DT = 1 / 30

FALL = "PointEmitter { position 0.3 2 -0.2 direction 0 -1 0 speed 3 variation 0 }"


def quad(y, half=1.0, solid=False):
    """A horizontal quad at height y around the fall line (its diagonal x = z misses the line x 0.3, z -0.2)."""
    return (f"IndexedFaceSet {{ solid {'TRUE' if solid else 'FALSE'} coord Coordinate {{ point [ {-half} {y} {-half}, "
            f"{half} {y} {-half}, {half} {y} {half}, {-half} {y} {half} ] }} coordIndex [ 0 3 2 1 -1 ] }}")


def planes(ys):
    """Horizontal quads at the heights ys in one IndexedFaceSet."""
    points = ", ".join(f"-1 {y:g} -1, 1 {y:g} -1, 1 {y:g} 1, -1 {y:g} 1" for y in ys)
    index = " ".join(f"{4 * i} {4 * i + 3} {4 * i + 2} {4 * i + 1} -1" for i in range(len(ys)))
    return f"IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ {points} ] }} coordIndex [ {index} ] }}"


def bm(geometry, damping=0.5, enabled=True, name="BM"):
    return (f"DEF {name} BoundedPhysicsModel {{ {'' if enabled else 'enabled FALSE '}damping {damping:g} "
            f"geometry {geometry} }}")


def ps(emitter, physics, n=4, life=2.0):
    return (f'DEF PS ParticleSystem {{\n  geometryType "POINT"\n  maxParticles {n}\n  particleLifetime {life:g}\n'
            f"  lifetimeVariation 0\n  emitter {emitter}\n  physics [ {' '.join(physics)} ]\n}}\n")


def switch_script(body, fields):
    return (f"DEF SW Script {{\n  inputOnly SFBool go\n{fields}  directOutput TRUE\n"
            f"  url \"ecmascript: function go (value) {{ if (value) {{ {body} }} }}\"\n}}\n")


# The event that runs the Script before frame 21 (a TimeSensor startTime between frames 20 and 21 ran it in neither
# X_ITE nor Rust under this protocol).
GO = {"events": [{"frame": 21, "node": "SW", "field": "go", "value": True}]}


ABOVE = lambda n: [3 + 0.05 * k for k in range(n)]
BELOW = lambda n: [-0.5 - 0.05 * k for k in range(n)]

CASES = {}

for hits in (31, 32, 33, 64):
    CASES[f"bp-hits-{hits}"] = (ps(FALL, [bm(planes([0] + ABOVE(hits - 1)))]), {})

for hits in (31, 32, 33, 34, 40, 48, 64, 128):
    CASES[f"bp-hits-{hits}-below"] = (ps(FALL, [bm(planes([0] + BELOW(hits - 1)))]), {})

# the floor listed last (written after the other hits), and the floor between planes above and below
CASES["bp-hits-64-floor-last"] = (ps(FALL, [bm(planes(BELOW(63) + [0]))]), {})
CASES["bp-hits-64-mixed"] = (ps(FALL, [bm(planes(ABOVE(32) + [0] + BELOW(31)))]), {})

CASES.update({
    # damping 0 on the floor; and the floor (damping 0) under a ceiling of damping 1 the bounced particles cannot reach
    "bp-damping-zero": (ps(FALL, [bm(quad(0), damping=0)]), {}),
    "bp-damping-zero-box": (ps("PointEmitter { position 0 0 0 direction 0 0 0 speed 3 variation 0 }",
                               [bm("Box { size 4 4 4 }", damping=0)], n=8, life=3.0), {}),
    "bp-damping-zero-mixed": (ps(FALL, [bm(quad(0), damping=0), bm(quad(4), damping=1, name="BM2")]), {}),
    # a floor turned 0.4 rad about Z in its own coordinates
    "bp-tilted": (ps(FALL, [bm("IndexedFaceSet { solid FALSE coord Coordinate { point [ -2 -0.8455 -2, 2 0.8455 -2, "
                               "2 0.8455 2, -2 -0.8455 2 ] } coordIndex [ 0 3 2 1 -1 ] }")]), {}),
    # the whole ParticleSystem under a rotated, non-uniformly scaled, translated Transform
    "bp-transformed": ("Transform { translation 0.5 -0.25 1 rotation 0 0 1 0.4 scale 1 2 0.5 children [\n"
                       + ps(FALL, [bm(quad(0))]) + "] }\n", {}),
    # per-vertex normals: the hit normal is interpolated over the triangle; its length is the damping
    "bp-normals": (ps(FALL, [bm("IndexedFaceSet { solid FALSE normalPerVertex TRUE coord Coordinate { point [ -1 0 -1, "
                                "1 0 -1, 1 0 1, -1 0 1 ] } normal Normal { vector [ 0.4 1 0, 0 1 0.3, -0.2 1 -0.5, "
                                "0.6 1 0.6 ] } coordIndex [ 0 3 2 1 -1 ] }", damping=0.8)]), {}),
    # a smooth Sphere around a spherical emitter
    "bp-normals-sphere": (ps("PointEmitter { position 0.1 0.05 0 direction 0 0 0 speed 3 variation 0 }",
                             [bm("Sphere { radius 1.5 }", damping=0.9)], n=8, life=3.0), {}),
    # enabled FALSE at frame 20, TRUE again at frame 40
    "bp-enabled-toggle": (ps(FALL, [bm(quad(0))]), {"frames": 75, "events": [
        {"frame": 20, "node": "BM", "field": "enabled", "value": False},
        {"frame": 40, "node": "BM", "field": "enabled", "value": True}]}),
    # enabled FALSE at load, TRUE at frame 20
    "bp-enabled-late": (ps(FALL, [bm(quad(0), enabled=False)]), {"events": [
        {"frame": 20, "node": "BM", "field": "enabled", "value": True}]}),
    # damping 0.5 -> 0.9 at frame 25
    "bp-damping-change": (ps(FALL, [bm(quad(0))]), {"events": [
        {"frame": 25, "node": "BM", "field": "damping", "value": 0.9}]}),
    # the floor's Coordinate moves up to y = 0.5 (X3DGeometryNode rebuild)
    "bp-rebuild": (ps(FALL, [bm("IndexedFaceSet { solid FALSE coord DEF CO Coordinate { point [ -1 0 -1, 1 0 -1, "
                                "1 0 1, -1 0 1 ] } coordIndex [ 0 3 2 1 -1 ] }")])
                   + switch_script("co .point = new MFVec3f (new SFVec3f (-1, 0.5, -1), new SFVec3f (1, 0.5, -1), "
                                   "new SFVec3f (1, 0.5, 1), new SFVec3f (-1, 0.5, 1));",
                                   "  inputOutput SFNode co USE CO\n"), GO),
    # BM.geometry is replaced by a floor at y = 0.7
    "bp-replace": (ps(FALL, [bm(quad(0))])
                   + switch_script("model .geometry = next;",
                                   "  inputOutput SFNode model USE BM\n  initializeOnly SFNode next "
                                   + quad(0.7) + "\n"), GO),
})


def timed_script(body, fields, test):
    """A looping TimeSensor TS (started at load) routes time to the Script SW, which acts when `test` holds."""
    return (f"DEF TS TimeSensor {{ loop TRUE startTime 1 }}\nDEF SW Script {{\n  inputOnly SFTime tick\n"
            f"  initializeOnly SFInt32 n 0\n  initializeOnly SFBool done FALSE\n{fields}  directOutput TRUE\n"
            f"  url \"ecmascript: function tick (value) {{ n = n + 1; if ({test}) {{ {body} }} }}\"\n}}\n"
            "ROUTE TS.time TO SW.tick\n")


FLOOR_COORD = ("IndexedFaceSet { solid FALSE coord DEF CO Coordinate { point [ -1 0 -1, 1 0 -1, 1 0 1, -1 0 1 ] } "
               "coordIndex [ 0 3 2 1 -1 ] }")


def lift(y):
    """The Script statement that moves the floor of FLOOR_COORD to height y."""
    return (f"co .point = new MFVec3f (new SFVec3f (-1, {y}, -1), new SFVec3f (1, {y}, -1), "
            f"new SFVec3f (1, {y}, 1), new SFVec3f (-1, {y}, 1));")


LIFT = lift(0.5)
# the Script chooses its time: the first tick at or after 20.5 frames from t0
START = repr(T0 + 20.5 * DT)
ONCE = f"!done && value >= {START}"
GEOM_FIELDS = "  inputOutput SFNode model USE BM\n  initializeOnly SFNode next " + quad(0.7) + "\n"

CASES.update({
    # A TimeSensor ST routes isActive to the Script. X_ITE schedules the start timeout only for a startTime EVENT (its
    # initialize reads the file value but never schedules it), so startTime is set by an event at frame 1 (the page's
    # virtual timers carry it); bp-ts-file-rebuild keeps the value in the file: ST never starts, the floor stays.
    "bp-ts-rebuild": (ps(FALL, [bm(FLOOR_COORD)]) + "DEF ST TimeSensor { }\n"
                      + switch_script(LIFT, "  inputOutput SFNode co USE CO\n") + "ROUTE ST.isActive TO SW.go\n",
                      {"events": [{"frame": 1, "node": "ST", "field": "startTime", "value": T0 + 20.5 * DT}]}),
    "bp-ts-replace": (ps(FALL, [bm(quad(0))]) + "DEF ST TimeSensor { }\n"
                      + switch_script("model .geometry = next;", GEOM_FIELDS) + "ROUTE ST.isActive TO SW.go\n",
                      {"events": [{"frame": 1, "node": "ST", "field": "startTime", "value": T0 + 20.5 * DT}]}),
    "bp-ts-file-rebuild": (ps(FALL, [bm(FLOOR_COORD)]) + f"DEF ST TimeSensor {{ startTime {START} }}\n"
                           + switch_script(LIFT, "  inputOutput SFNode co USE CO\n") + "ROUTE ST.isActive TO SW.go\n", {}),
    # a looping TimeSensor ticks the Script, which acts once, at a time it chooses from the tick's time
    "bp-tick-rebuild": (ps(FALL, [bm(FLOOR_COORD)])
                        + timed_script("done = true; " + LIFT, "  inputOutput SFNode co USE CO\n", ONCE), {}),
    "bp-tick-replace": (ps(FALL, [bm(quad(0))])
                        + timed_script("done = true; model .geometry = next;", GEOM_FIELDS, ONCE), {}),
    # the Script rebuilds the floor on every 12th tick (up, down, up ...)
    "bp-tick-repeat": (ps(FALL, [bm(FLOOR_COORD)], life=2.5)
                       + timed_script("done = !done; y = done ? 0.5 : 0; co .point = new MFVec3f (new SFVec3f (-1, y, -1), "
                                      "new SFVec3f (1, y, -1), new SFVec3f (1, y, 1), new SFVec3f (-1, y, 1));",
                                      "  inputOutput SFNode co USE CO\n", "n % 12 == 0"), {"frames": 75}),
    # the Script builds the new geometry from a string when it chooses
    "bp-tick-create": (ps(FALL, [bm(quad(0))])
                       + timed_script("done = true; model .geometry = Browser .createVrmlFromString ('#VRML V2.0 utf8' + "
                                      "String .fromCharCode (10) + 'Shape { geometry Box { size 3 0.2 3 } }') [0] .geometry;",
                                      "  inputOutput SFNode model USE BM\n", ONCE), {}),
})


# One frame decides: the falling particle is at y 1.491 after frame 22 and 1.411 after frame 23; a floor lifted to 1.45
# is hit only when the change is made before the particle step of frame 23 (a hit), not from frame 24 on (a miss).
# X_ITE's start timeout of an event at frame 1 fires in the first step whose time is at or after the startTime
# (measured: 23.0 frames starts the sensor in step 23, 23.1 frames in step 24), so 22.9 is a hit and 23.1 a miss.
LATE = lift(1.45)


def ts_late(frames_start):
    return (ps(FALL, [bm(FLOOR_COORD)]) + "DEF ST TimeSensor { }\n"
            + switch_script(LATE, "  inputOutput SFNode co USE CO\n") + "ROUTE ST.isActive TO SW.go\n",
            {"events": [{"frame": 1, "node": "ST", "field": "startTime", "value": T0 + frames_start * DT}]})


CASES.update({
    "bp-ts-hit": ts_late(22.9),
    "bp-ts-miss": ts_late(23.1),
    "bp-tick-hit": (ps(FALL, [bm(FLOOR_COORD)]) + timed_script(
        "done = true; " + LATE, "  inputOutput SFNode co USE CO\n", f"!done && value >= {T0 + 22.5 * DT!r}"), {}),
    "bp-tick-miss": (ps(FALL, [bm(FLOOR_COORD)]) + timed_script(
        "done = true; " + LATE, "  inputOutput SFNode co USE CO\n", f"!done && value >= {T0 + 23.5 * DT!r}"), {}),
})


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)

    for i, (name, (body, extra)) in enumerate(CASES.items()):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + body)

        script = {"dt": DT, "t0": T0, "frames": 60, "warmup": 8, "seed": 16000000 + i, "node": "PS", "dumpAll": True}
        script.update(extra)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(script, f)

    print(f"{len(CASES)} cases in {ROOT}")


main()
