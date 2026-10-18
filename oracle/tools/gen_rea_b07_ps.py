#!/usr/bin/env python3
"""REA batch 07 (particles part): generate the ForcePhysicsModel / WindPhysicsModel / PolylineEmitter worlds and
driver scripts.

  python3 -I oracle/tools/gen_rea_b07_ps.py

Writes oracle/synthetic-rea-b07-ps/ps-NAME.x3dv and oracle/synthetic-rea-b07-ps/scripts/ps-NAME.json for every
case of CASES (deterministic: running it twice changes nothing). The scripts are read by
oracle/tools/x_ite_particles_b07.mjs (X_ITE) and by x3d-particles-drive (Rust); both understand the "events" list.

Conventions
-----------
* Every world is `#X3D V4.0 utf8` EXCEPT the deliberate legacy cases (`legacy` key): X_ITE's WindPhysicsModel
  constructor makes `direction` default to the zero vector when the scene's specification version is <= 3.3
  (so `#X3D V3.3` worlds, like the 06R particle worlds, would silently give every Wind a random direction).
* Node names: PS the ParticleSystem, EM the emitter, F1 F2 ... ForcePhysicsModels, W1 W2 ... WindPhysicsModels,
  C the emitter's Coordinate, C2 a spare Coordinate (held by a Shape so events can swap it in).
* The frame rate of a case is `rate` (the script's dt is 1 / rate); the time-dependent cases exist at several
  rates (24, 60 and 144 Hz for the core cases, more for some), see RATES.
* Every case is compared per particle (positions, velocities, lifetimes ...) against X_ITE with the recorded
  Math.random stream of X_ITE (the per-frame emitter seed and the draws of the Wind models), so no case needs a
  statistical test: the only randomness X_ITE has is Math.random, and it is replayed.
"""
import json, os, zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
wdir = os.path.join(root, "oracle", "synthetic-rea-b07-ps")
sdir = os.path.join(wdir, "scripts")

RATES = (24, 30, 50, 60, 75, 120, 144)
CORE_RATES = (24, 60, 144)

CASES = {}


def case(name, emitter, physics=(), n=12, life=1.0, var=0.25, secs=1.5, rate=60, ver="4.0", create=True, events=(), dump=None,
         extra="", defs="", note="", legacy=False, frames=None, tol=None, group=False):
    CASES[name] = dict(emitter=emitter, physics=list(physics), n=n, life=life, var=var, secs=secs, rate=rate, ver=ver, create=create,
                       events=list(events), dump=dump, extra=extra, defs=defs, note=note, legacy=legacy, frames=frames, tol=tol, group=group)


def rates(base, emitter, physics=(), rate_list=CORE_RATES, **kw):
    for r in rate_list:
        case("%s-r%d" % (base, r), emitter, physics, rate=r, **kw)


def pts(points):
    return ", ".join("%g %g %g" % p for p in points)


def coord(points, name=None, double=False):
    t = "CoordinateDouble" if double else "Coordinate"
    return "%s%s { point [ %s ] }" % ("DEF %s " % name if name else "", t, pts(points))


def idx(l):
    return " ".join(str(i) for i in l)


def point_emitter(extra="", **kw):
    s = "PointEmitter { position 0 1 0 direction 1 0 0 speed 2 variation 0.25 mass 1 %s}" % extra
    return s


# ------------------------------------------------------------------------------------------------ emitters
PT = "DEF EM PointEmitter { position 0 1 0 direction 1 0 0 speed 2 variation 0.25 mass 1 }"
PT_SA = "DEF EM PointEmitter { position 0 1 0 direction 0 1 0 speed 1 variation 0.25 mass 1 surfaceArea 4 }"
CONE = "DEF EM ConeEmitter { position 0 0 0 direction 0 1 0 angle 0.6 speed 2 variation 0.25 mass 1 surfaceArea 2 }"
EXPL = "DEF EM ExplosionEmitter { position 0 1 0 speed 3 variation 0.25 mass 1 surfaceArea 2 }"
SURF = ("DEF EM SurfaceEmitter { speed 1 variation 0.25 mass 1 surfaceArea 2 surface IndexedFaceSet { solid FALSE coord Coordinate "
        "{ point [ 0 0 0, 2 0 0, 2 1 0, 0 1 0, 3 0 0, 3 1 0, 3 0 1 ] } coordIndex [ 0 1 2 3 -1 4 5 6 -1 ] } }")
VOL = ("DEF EM VolumeEmitter { direction 0 1 0 speed 1 variation 0.25 mass 1 surfaceArea 2 coordIndex [ 0 3 2 1 -1 4 5 6 7 -1 0 1 5 4 -1 2 3 7 6 -1 1 2 6 5 -1 0 4 7 3 -1 ] "
       "coord Coordinate { point [ -1 -1 -1, 1 -1 -1, 1 1 -1, -1 1 -1, -1 -1 1, 1 -1 1, 1 1 1, -1 1 1 ] } }")
FLOOR = ("BoundedPhysicsModel { damping 0.6 geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -10 0 -10, 10 0 -10, 10 0 10, -10 0 10 ] } "
         "coordIndex [ 0 3 2 1 -1 ] } }")
FALL = "DEF EM PointEmitter { position 0.3 2 -0.2 direction 0 0 0 speed 0.5 variation 0.5 mass 1 surfaceArea 3 }"


def force(name="F1", f=None, enabled=True):
    return "DEF %s ForcePhysicsModel { %s%s}" % (name, "" if enabled else "enabled FALSE ", "" if f is None else "force %s " % " ".join("%g" % x for x in f))


def wind(name="W1", enabled=True, direction=None, speed=None, gustiness=None, turbulence=None):
    s = "DEF %s WindPhysicsModel { " % name
    if not enabled: s += "enabled FALSE "
    if direction is not None: s += "direction %s " % " ".join("%g" % x for x in direction)
    if speed is not None: s += "speed %g " % speed
    if gustiness is not None: s += "gustiness %g " % gustiness
    if turbulence is not None: s += "turbulence %g " % turbulence
    return s + "}"


# A Coordinate no emitter uses yet, held by a Shape so that events can swap it into an emitter's coord.
SPARE4 = "Shape { geometry IndexedLineSet { coordIndex [ 0 1 2 3 -1 ] coord DEF C2 Coordinate { point [ 10 0 0, 10 5 0, 12 5 0, 12 5 5 ] } } }"
SPARE3 = "Shape { geometry IndexedLineSet { coordIndex [ 0 1 2 -1 ] coord DEF C2 Coordinate { point [ 10 0 0, 10 5 0, 12 5 0 ] } } }"


def ev(frame, node, field, value):
    return {"frame": frame, "node": node, "field": field, "value": value}


# ------------------------------------------------------------------------------------------------ ForcePhysicsModel
rates("force-default", PT, [force()])
rates("force-two", PT, [force("F1", (1, 2, -3)), force("F2", (0, -9.8, 0))], rate_list=(24, 60, 144), life=1.5)
case("force-mass0", "DEF EM PointEmitter { position 0 1 0 direction 1 0 0 speed 2 variation 0.25 }", [force()], note="mass 0 (default): no force at all, numForces 0")
case("force-massneg", "DEF EM PointEmitter { position 0 1 0 direction 1 0 0 speed 2 mass -1 }", [force()], note="mass is clamped to 0")
case("force-disabled-first", PT, [force("F1", (0, 3, 0), enabled=False), force("F2", (1, 0, 0))], note="the disabled model adds no row; the enabled one is row 0")
case("force-all-disabled", PT, [force("F1", enabled=False)], note="numForces 0")
case("force-zero", PT, [force("F1", (0, 0, 0))], note="a zero force still costs two random draws per particle per frame in the shader (turbulence 0)")
case("force-big-mass", "DEF EM PointEmitter { position 0 1 0 direction 1 0 0 speed 2 variation 0.25 mass 25 }", [force("F1", (0, -9.8, 0))])
case("force-default-emitter", None, [force()], note="no emitter field: X_ITE's default PointEmitter, mass 0, so no force")
case("force-cone", CONE, [force("F1", (0, -9.8, 0))])
case("force-explosion", EXPL, [force("F1", (0, -9.8, 0))], n=16, secs=3.0, note="deltaTime is Infinity on the burst frame: the force rows hold inf, no particle reads them")
case("force-surface", SURF, [force("F1", (0, -2, 0))])
case("force-volume", VOL, [force("F1", (0, -2, 0))])
case("force-bounded", FALL, [force("F1", (0, -9.8, 0)), FLOOR], life=3.0, var=0, secs=3.0, note="Force plus BoundedPhysicsModel")
case("force-events-r60", PT, [force("F1"), force("F2", (0, 1, 0))], life=1.5, secs=3.0, events=[
    ev(20, "F1", "force", [0, -2, 5]), ev(35, "F1", "enabled", False), ev(50, "F1", "enabled", True), ev(60, "EM", "mass", 0), ev(75, "EM", "mass", 2),
    ev(90, "F2", "enabled", False), ev(105, "F2", "force", [4, 4, 4]), ev(110, "F2", "enabled", True), ev(130, "EM", "speed", 0), ev(150, "EM", "variation", 0)],
     note="force, enabled, mass, speed changes at run time")
case("force-events-r24", PT, [force("F1"), force("F2", (0, 1, 0))], rate=24, life=1.5, secs=4.0, events=[
    ev(10, "F1", "force", [0, -2, 5]), ev(20, "F1", "enabled", False), ev(30, "F1", "enabled", True), ev(40, "EM", "mass", 0), ev(50, "EM", "mass", 2), ev(60, "F2", "enabled", False)])
case("force-events-r144", PT, [force("F1"), force("F2", (0, 1, 0))], rate=144, life=1.5, secs=2.5, events=[
    ev(50, "F1", "force", [0, -2, 5]), ev(100, "F1", "enabled", False), ev(150, "F1", "enabled", True), ev(200, "EM", "mass", 0), ev(250, "EM", "mass", 2), ev(300, "F2", "enabled", False)])
case("force-createoff", PT, [force()], create=False, note="createParticles FALSE: hidden births, forces still computed")

# ------------------------------------------------------------------------------------------------ WindPhysicsModel
rates("wind-default", PT_SA, [wind()], note="all defaults (speed 0.1, gustiness 0.1, direction 1 0 0, turbulence 0)")
rates("wind-dir0", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)], note="direction 0: getRandomNormal per frame")
rates("wind-gust", PT_SA, [wind("W1", direction=(1, 0.5, -0.2), speed=1.2, gustiness=0.5, turbulence=0.8)])
case("wind-turbulence-max", PT_SA, [wind("W1", direction=(0, 1, 0), speed=1.5, gustiness=0, turbulence=1)], note="turbulence 1: pi")
case("wind-turbulence-clamp", PT_SA, [wind("W1", direction=(0, 1, 0), speed=1.5, gustiness=0, turbulence=5)], note="turbulence 5 clamps to 1")
case("wind-speed0", PT_SA, [wind("W1", direction=(1, 0, 0), speed=0, gustiness=1, turbulence=0.5)], note="speed 0: pressure 10 ** (2 * log (0)) = 0, but the draws still happen")
case("wind-speed-neg", PT_SA, [wind("W1", direction=(1, 0, 0), speed=-2, gustiness=-1, turbulence=-1)], note="negative speed, gustiness and turbulence clamp to 0")
case("wind-area-neg", "DEF EM PointEmitter { position 0 1 0 direction 0 1 0 speed 1 variation 0.25 mass 1 surfaceArea -3 }", [wind("W1", speed=1.5)], note="negative surfaceArea clamps to 0")
case("wind-area0", "DEF EM PointEmitter { position 0 1 0 direction 0 1 0 speed 1 variation 0.25 mass 1 }", [wind("W1", speed=1.5)], note="surfaceArea 0 (default): zero force, but the draws happen")
case("wind-mass0", "DEF EM PointEmitter { position 0 1 0 direction 0 1 0 speed 1 variation 0.25 surfaceArea 4 }", [wind("W1", speed=1.5)], note="mass 0: addForce is never called, no draws")
case("wind-strong", PT_SA, [wind("W1", direction=(0, 0, 0), speed=3, gustiness=0.3, turbulence=0.3)], tol="strong",
     note="speed 3: |force| about 7 per frame at 60 Hz; the GPU's turbulence normal carries ~2e-7 relative noise per step, see the report")
case("wind-strong-r24", PT_SA, [wind("W1", direction=(1, 0, 0), speed=3, gustiness=0.3, turbulence=0.6)], rate=24, tol="strong")
case("wind-strong-r144", PT_SA, [wind("W1", direction=(0, 1, 0), speed=3, gustiness=0.3, turbulence=0.6)], rate=144, tol="strong")
case("wind-dirraw", PT_SA, [wind("W1", direction=(3, 0, 4), speed=1.5, gustiness=0.2)], note="direction is normalized on the CPU")
case("wind-disabled", PT_SA, [wind("W1", enabled=False, speed=1.5)], note="enabled FALSE: no draws")
case("wind-multi", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.2), force("F1", (0, -1, 0)), wind("W2", enabled=False),
                           wind("W3", direction=(0, 1, 0), speed=1.0, gustiness=0.5, turbulence=0.4)], note="wind, force, disabled wind, wind: rows 0 1 2")
case("wind-cone", CONE, [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)])
case("wind-explosion", EXPL, [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)], n=16, secs=3.0, note="burst: deltaTime Infinity, wind rows NaN / inf, nobody reads them")
case("wind-surface", SURF, [wind("W1", direction=(1, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)])
case("wind-volume", VOL, [wind("W1", direction=(1, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)])
case("wind-bounded", FALL, [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.2), FLOOR], life=3.0, var=0, secs=3.0)
case("wind-createoff", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.5)], create=False)
case("wind-legacy33-nodir", PT_SA, [wind("W1", speed=1.5, gustiness=0.3, turbulence=0.3)], ver="3.3", legacy=True,
     note="#X3D V3.3: direction defaults to 0 (random normal each frame), 3 draws per frame")
case("wind-legacy33-dir", PT_SA, [wind("W1", direction=(1, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)], ver="3.3", legacy=True,
     note="#X3D V3.3 with an explicit direction in the file: honored (1 draw per frame)")
case("wind-legacy32-nodir", PT_SA, [wind("W1", speed=1.5, gustiness=0.3, turbulence=0.3)], ver="3.2", legacy=True)
case("wind-legacy33-events", PT_SA, [wind("W1", speed=1.5, gustiness=0.3, turbulence=0.3)], ver="3.3", legacy=True, secs=2.0, events=[
    ev(20, "W1", "direction", [0, 1, 0]), ev(40, "W1", "direction", [0, 0, 0]), ev(60, "W1", "direction", [1, 0, 0])], note="direction events in a legacy scene")
case("wind-modern-nodir-events", PT_SA, [wind("W1", speed=1.5, gustiness=0.3, turbulence=0.3)], secs=2.0, events=[
    ev(20, "W1", "direction", [0, 0, 0]), ev(40, "W1", "direction", [0, 1, 0])], note="V4.0: default direction 1 0 0")
case("wind-events-r60", PT_SA, [wind("W1", direction=(1, 0, 0), speed=1.2, gustiness=0.1, turbulence=0.1), wind("W2", direction=(0, 0, 0), speed=1)], secs=3.0, events=[
    ev(15, "W1", "speed", 1.4), ev(30, "W1", "gustiness", 0.3), ev(40, "W1", "turbulence", 0.9), ev(50, "W1", "direction", [0, 0, 0]),
    ev(60, "W1", "direction", [0, 0, 1]), ev(70, "W2", "enabled", False), ev(80, "W1", "enabled", False), ev(90, "W2", "enabled", True),
    ev(100, "W1", "enabled", True), ev(110, "EM", "surfaceArea", 4), ev(120, "EM", "mass", 0), ev(130, "EM", "mass", 3), ev(140, "W2", "speed", 0),
    ev(150, "W2", "speed", 1.3), ev(160, "EM", "surfaceArea", 0), ev(170, "EM", "surfaceArea", 1)], note="every inputOutput field of WindPhysicsModel and the emitter's mass / surfaceArea")
case("wind-events-r24", PT_SA, [wind("W1", direction=(1, 0, 0), speed=1.2, gustiness=0.1, turbulence=0.1)], rate=24, secs=4.0, events=[
    ev(10, "W1", "speed", 1.4), ev(20, "W1", "direction", [0, 0, 0]), ev(30, "W1", "turbulence", 0.9), ev(40, "W1", "enabled", False), ev(50, "W1", "enabled", True),
    ev(60, "W1", "gustiness", 0.3), ev(70, "EM", "surfaceArea", 4)])
case("wind-events-r144", PT_SA, [wind("W1", direction=(1, 0, 0), speed=1.2, gustiness=0.1, turbulence=0.1)], rate=144, secs=2.5, events=[
    ev(40, "W1", "speed", 1.4), ev(80, "W1", "direction", [0, 0, 0]), ev(120, "W1", "turbulence", 0.9), ev(160, "W1", "enabled", False), ev(200, "W1", "enabled", True),
    ev(240, "W1", "gustiness", 0.3), ev(280, "EM", "surfaceArea", 4)])
case("wind-big", "DEF EM PointEmitter { position 0 1 0 direction 0 1 0 speed 1 variation 0.25 mass 1 surfaceArea 4 }", [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3)],
     n=20000, life=0.5, secs=1.0, dump=[10, 30], note="20000 particles: the per-particle states after 10 and 30 frames")

# ------------------------------------------------------------------------------------------------ PolylineEmitter
P4 = [(0, 0, 0), (1, 0, 0), (1, 2, 0), (3, 2, 1)]


def polyline(coord_index, points=P4, extra="", double=False, nocoord=False):
    return "DEF EM PolylineEmitter { direction 0 1 0 speed 1 variation 0.25 %scoordIndex [ %s ] %s}" % (
        extra + " " if extra else "", idx(coord_index), "" if nocoord else "coord %s " % coord(points, "C", double))


rates("polyline-basic", polyline([0, 1, 2, 3, -1]), life=1.0)
case("polyline-default", "DEF EM PolylineEmitter { }", note="coordIndex [-1], no coord: verticesIndex -1, hidden (NaN) births")
case("polyline-default-world", "DEF EM PolylineEmitter { direction 0 1 0 speed 1 }", note="defaults of everything but speed")
case("polyline-multi", polyline([0, 1, 2, -1, 3, -1, 4, 5, -1, 6, 7, 8, 9, -1], points=[(0, 0, 0), (1, 0, 0), (1, 3, 0), (5, 5, 5), (-2, 0, 0), (-2, 0, 4), (0, -1, 0), (1, -1, 0), (1, -1, 2), (4, -1, 2)]),
     n=24, note="four polylines with segments of different lengths and a one-point polyline (skipped)")
case("polyline-multi-r24", polyline([0, 1, 2, -1, 3, -1, 4, 5, -1, 6, 7, 8, 9, -1], points=[(0, 0, 0), (1, 0, 0), (1, 3, 0), (5, 5, 5), (-2, 0, 0), (-2, 0, 4), (0, -1, 0), (1, -1, 0), (1, -1, 2), (4, -1, 2)]), rate=24, n=24)
case("polyline-multi-r144", polyline([0, 1, 2, -1, 3, -1, 4, 5, -1, 6, 7, 8, 9, -1], points=[(0, 0, 0), (1, 0, 0), (1, 3, 0), (5, 5, 5), (-2, 0, 0), (-2, 0, 4), (0, -1, 0), (1, -1, 0), (1, -1, 2), (4, -1, 2)]), rate=144, n=24)
case("polyline-dir0", polyline([0, 1, 2, 3, -1]).replace("direction 0 1 0", "direction 0 0 0"), note="direction 0: spherical velocity")
case("polyline-dirraw", polyline([0, 1, 2, 3, -1]).replace("direction 0 1 0", "direction 0 3 4"), note="direction is normalized")
case("polyline-nondefault", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 2").replace("speed 1 variation 0.25", "speed 2.5 variation 0.6"), [force("F1", (0, -2, 0))])
case("polyline-fixed", polyline([0, 1, 2, 3, -1]).replace("variation 0.25", "variation 0"), var=0, note="no variation: seed independent velocity")
case("polyline-nospeed", polyline([0, 1, 2, 3, -1]).replace("speed 1", "speed 0"))
case("polyline-off", polyline([0, 1, 2, 3, -1], extra="on FALSE"), note="on FALSE: hidden births")
case("polyline-createoff", polyline([0, 1, 2, 3, -1]), create=False)
case("polyline-single-point", polyline([0, -1]), note="one-point polyline: no segment, no vertices, NaN")
case("polyline-empty-coord", "DEF EM PolylineEmitter { direction 0 1 0 speed 1 coordIndex [ 0 1 -1 ] coord DEF C Coordinate { } }", note="empty coord: the IndexedLineSet builds nothing")
case("polyline-minus1-only", polyline([-1]), note="coordIndex [-1] with points")
case("polyline-empty-index", polyline([]), note="coordIndex [] with points")
case("polyline-nocoord", polyline([0, 1, 2, -1], nocoord=True), note="coordIndex without coord")
case("polyline-no-terminator", polyline([0, 1, 2]), note="no trailing -1: the last polyline is still taken")
case("polyline-bad-index", polyline([0, 99, 1, -1]), note="index past coord: the origin")
case("polyline-minus2", polyline([0, 1, -2, 2, 3, -1]), note="any negative index ends a polyline")
case("polyline-zero-length", polyline([0, 0, 0, -1]), note="three equal points: total length 0, fraction 0, first segment")
case("polyline-zero-and-real", polyline([0, 0, -1, 0, 1, 2, -1]), note="a zero-length polyline next to a real one")
case("polyline-closed", polyline([0, 1, 2, 0, -1]), note="a polyline that returns to its first point")
case("polyline-double", polyline([0, 1, 2, 3, -1], points=[(0, 0, 0), (1.123456789012, 0, 0), (1, 2, 0.000000123456789), (3, 2, 1)], double=True), note="CoordinateDouble: doubles are rounded to float only in the texture")
case("polyline-two-points", polyline([0, 3, -1]), note="a single segment")
case("polyline-bounded", "DEF EM PolylineEmitter { direction 0 -1 0 speed 3 variation 0.25 mass 1 coordIndex [ 0 1 -1 ] coord DEF C Coordinate { point [ -1 3 0, 1 3 0.5 ] } }", [FLOOR, force("F1", (0, -9.8, 0))],
     life=2.0, secs=3.0, note="polyline + gravity + bounded floor")
case("polyline-force-wind", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 4"), [wind("W1", direction=(0, 0, 0), speed=1.5, gustiness=0.3, turbulence=0.3), force("F1", (0, -1, 0))])
case("polyline-dir0-wind", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 4").replace("direction 0 1 0", "direction 0 0 0"), [wind("W1", direction=(1, 0, 0), speed=1.5, gustiness=0.3)])
case("polyline-events-r60", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 2"), [force("F1", (0, -1, 0))], secs=3.0,
     defs=SPARE4, events=[
         ev(15, "EM", "set_coordIndex", [0, 1, -1]), ev(30, "C", "point", [0, 0, 0, 2, 0, 0, 2, 3, 0, 0, 3, 3]), ev(45, "EM", "set_coordIndex", [0, 1, -1, 2, 3, -1]),
         ev(60, "EM", "coord", {"def": "C2"}), ev(75, "EM", "set_coordIndex", [0, 1, 2, 3, -1]), ev(90, "EM", "coord", {"def": None}), ev(105, "EM", "coord", {"def": "C"}),
         ev(120, "EM", "direction", [0, 0, 0]), ev(130, "EM", "direction", [1, 0, 0]), ev(140, "EM", "speed", 3), ev(150, "EM", "variation", 0.9), ev(160, "EM", "on", False), ev(170, "EM", "on", True),
         ev(175, "EM", "set_coordIndex", [-1]), ev(180, "EM", "set_coordIndex", [3, 2, 1, 0, -1])],
     note="set_coordIndex, Coordinate.point, coord replaced by another node, coord NULL, direction, speed, variation, on")
case("polyline-events-r24", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 2"), [force("F1", (0, -1, 0))], rate=24, secs=4.0,
     defs=SPARE4, events=[
         ev(10, "EM", "set_coordIndex", [0, 1, -1]), ev(20, "C", "point", [0, 0, 0, 2, 0, 0, 2, 3, 0, 0, 3, 3]), ev(30, "EM", "coord", {"def": "C2"}), ev(40, "EM", "coord", {"def": None}),
         ev(50, "EM", "coord", {"def": "C"}), ev(60, "EM", "direction", [0, 0, 0]), ev(70, "EM", "on", False), ev(80, "EM", "on", True)])
case("polyline-events-r144", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 2"), [force("F1", (0, -1, 0))], rate=144, secs=2.5,
     defs=SPARE4, events=[
         ev(40, "EM", "set_coordIndex", [0, 1, -1]), ev(80, "C", "point", [0, 0, 0, 2, 0, 0, 2, 3, 0, 0, 3, 3]), ev(120, "EM", "coord", {"def": "C2"}), ev(160, "EM", "coord", {"def": None}),
         ev(200, "EM", "coord", {"def": "C"}), ev(240, "EM", "direction", [0, 0, 0]), ev(280, "EM", "on", False), ev(320, "EM", "on", True)])
case("polyline-from-empty", "DEF EM PolylineEmitter { direction 0 1 0 speed 1 }", secs=2.0,
     defs=SPARE3, events=[
         ev(20, "EM", "set_coordIndex", [0, 1, 2, -1]), ev(40, "EM", "coord", {"def": "C2"}), ev(60, "C2", "point", [0, 0, 0, 1, 1, 1, 2, 0, 0])],
     note="starts with nothing (hidden births), then coordIndex, then coord")
case("polyline-big", polyline([0, 1, -1, 2, 3, -1, 4, 5, -1], points=[(0, 0, 0), (1, 0, 0), (0, 1, 0), (3, 1, 0), (0, 2, 0), (6, 2, 0)]), n=20000, life=0.5, secs=1.0, dump=[10, 30],
     note="20000 particles on three segments of length 1, 3, 6: births are proportional to the length")

# Removal from a live Group: a ParticleSystem is not traversed any more, but its liveness is that of its execution
# context (BaseNode.setLive via the context, not the parent), so X_ITE keeps animating it.
case("polyline-group-removal", polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 2"), [wind("W1", direction=(0, 0, 0), speed=1.2, gustiness=0.2, turbulence=0.2), force("F1", (0, -1, 0))], group=True, secs=2.0,
     events=[ev(30, "GRP", "children", {"nodes": []}), ev(70, "GRP", "children", {"nodes": ["PS"]})],
     note="the system leaves the Group at frame 30 and returns at frame 70")
case("wind-group-removal-r144", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.2, gustiness=0.2, turbulence=0.2)], group=True, rate=144, secs=1.5,
     events=[ev(60, "GRP", "children", {"nodes": []}), ev(120, "GRP", "children", {"nodes": ["PS"]})])

# Field changes made by the WORLD (ROUTEs and a Script), not by the harness: the same world runs in X_ITE and in Rust.
TIMER = "DEF TS TimeSensor { cycleInterval 1 loop TRUE }\n"
ROUTES_WIND = (TIMER +
               "DEF SI ScalarInterpolator { key [ 0 0.5 1 ] keyValue [ 1.0 1.5 1.0 ] }\n"
               "DEF GI ScalarInterpolator { key [ 0 1 ] keyValue [ 0 0.6 ] }\n"
               "DEF TI ScalarInterpolator { key [ 0 0.5 1 ] keyValue [ 0 0.8 0 ] }\n"
               "DEF BT BooleanToggle { }\n"
               "ROUTE TS.fraction_changed TO SI.set_fraction\nROUTE SI.value_changed TO W1.speed\n"
               "ROUTE TS.fraction_changed TO GI.set_fraction\nROUTE GI.value_changed TO W1.gustiness\n"
               "ROUTE TS.fraction_changed TO TI.set_fraction\nROUTE TI.value_changed TO W1.turbulence\n"
               "ROUTE TS.cycleTime TO BT.set_boolean\nROUTE BT.toggle_changed TO W1.enabled\n")
case("route-wind-r60", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.0, gustiness=0.1, turbulence=0.1)], secs=3.0, defs=ROUTES_WIND,
     note="speed, gustiness, turbulence and enabled of a WindPhysicsModel driven by ROUTEs (interpolators, BooleanToggle)")
case("route-wind-r24", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.0, gustiness=0.1, turbulence=0.1)], rate=24, secs=3.0, defs=ROUTES_WIND)
case("route-wind-r144", PT_SA, [wind("W1", direction=(0, 0, 0), speed=1.0, gustiness=0.1, turbulence=0.1)], rate=144, secs=3.0, defs=ROUTES_WIND)

SCRIPT_POLY = (TIMER + "DEF SC Script { directOutput TRUE inputOnly SFTime set_time initializeOnly SFNode w USE W1 initializeOnly SFNode em USE EM initializeOnly SFNode co USE C initializeOnly SFNode co2 USE C2\n"
               "  initializeOnly SFFloat t0 -1 initializeOnly SFInt32 stage 0\n"
               "  url \"ecmascript:\n"
               "    function set_time (v) { if (t0 < 0) t0 = v; var d = v - t0;\n"
               "      if (stage == 0 && d > 0.3) { stage = 1; w .direction = new SFVec3f (0, 0, 0); }\n"
               "      else if (stage == 1 && d > 0.6) { stage = 2; em .set_coordIndex = new MFInt32 (0, 1, -1, 2, 3, -1); }\n"
               "      else if (stage == 2 && d > 0.9) { stage = 3; co .point = new MFVec3f (new SFVec3f (0, 0, 0), new SFVec3f (2, 0, 0), new SFVec3f (2, 3, 0), new SFVec3f (0, 3, 3)); }\n"
               "      else if (stage == 3 && d > 1.2) { stage = 4; em .coord = co2; em .set_coordIndex = new MFInt32 (0, 1, 2, -1); }\n"
               "      else if (stage == 4 && d > 1.5) { stage = 5; em .speed = 3; em .variation = 0.5; w .enabled = false; }\n"
               "      else if (stage == 5 && d > 1.8) { stage = 6; em .direction = new SFVec3f (0, 0, 0); w .enabled = true; w .direction = new SFVec3f (1, 0, 0); }\n"
               "      else if (stage == 6 && d > 2.1) { stage = 7; em .coord = null; }\n"
               "      else if (stage == 7 && d > 2.4) { stage = 8; em .coord = co; em .set_coordIndex = new MFInt32 (3, 2, 1, 0, -1); }\n"
               "    }\"\n"
               "}\nROUTE TS.time TO SC.set_time\n")
for r in CORE_RATES:
    case("script-polyline-r%d" % r, polyline([0, 1, 2, 3, -1], extra="mass 1 surfaceArea 3"), [wind("W1", direction=(1, 0, 0), speed=1.2, gustiness=0.2, turbulence=0.2), force("F1", (0, -1, 0))],
         rate=r, secs=3.0, defs=SPARE4 + "\n" + SCRIPT_POLY, note="a Script changes direction, set_coordIndex, Coordinate.point, coord, speed, variation, enabled, direction, coord NULL")

# ------------------------------------------------------------------------------------------------ files

def world(c):
    ps = ["geometryType \"POINT\"", "maxParticles %d" % c["n"], "particleLifetime %g" % c["life"], "lifetimeVariation %g" % c["var"]]
    if not c["create"]:
        ps.append("createParticles FALSE")
    if c["emitter"]:
        ps.append("emitter %s" % c["emitter"])
    if c["physics"]:
        ps.append("physics [ %s ]" % " ".join(c["physics"]))
    head = "#X3D V%s utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n\n" % c["ver"]
    body = "DEF PS ParticleSystem {\n  %s\n}\n" % "\n  ".join(ps)
    if c["group"]:
        body = "DEF GRP Group { children [\n%s] }\n" % body
    return head + body + ("\n" + c["defs"] + "\n" if c["defs"] else "")


def script(name, c):
    frames = c["frames"] or int(round(c["secs"] * c["rate"]))
    s = {"dt": 1 / c["rate"], "t0": 946684800, "frames": frames, "warmup": 8, "seed": zlib.crc32(name.encode()) & 0x7fffffff, "node": "PS"}
    if c["dump"]:
        s["dump"] = c["dump"]
    else:
        s["dumpAll"] = True
    if c["events"]:
        s["events"] = c["events"]
    if c["tol"]:
        s["tol"] = c["tol"]     # read by rea_b07_ps.py and by crates/x3d-viewer/tests/particles_b07.rs
    return s


if __name__ == "__main__":
    os.makedirs(sdir, exist_ok=True)
    for name, c in sorted(CASES.items()):
        with open(os.path.join(wdir, "ps-%s.x3dv" % name), "w") as f:
            f.write(world(c))
        with open(os.path.join(sdir, "ps-%s.json" % name), "w") as f:
            json.dump(script(name, c), f)
    print("%d cases written to %s" % (len(CASES), wdir))
