#!/usr/bin/env python3
"""REA batch 06R: generate the ParticleSystems emitter worlds and driver scripts.

  python3 -I oracle/tools/gen_rea_b06_particles.py

Writes oracle/synthetic-rea-b06/particles-NAME.x3dv and oracle/synthetic-rea-b06/scripts/particles-NAME.json
for every case in CASES. Small cases ("exact") have a handful of particles and dump every frame, for a
per-particle comparison given the recorded GLSL seeds. Cases named NAME-stat have 20000 particles and
dump only the particles created in each frame ("filter": "born"), for a distribution comparison.
The files are deterministic; running the generator twice changes nothing.

CASES is also the structured description of each world (rea_b06_particles.py imports this module for its
reference emulation and for the analytic checks): every case is a dict with the emitter spec "E", the
physics list "P" and the ParticleSystem fields.
"""
import json, os, zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
wdir = os.path.join(root, "oracle", "synthetic-rea-b06")
sdir = os.path.join(wdir, "scripts")

DT = 1 / 30
HEAD = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n\n"
PI = 3.14159265

# ---- explicit geometry specs: ("ifs", points, coordIndex, solid) | ("box", (sx, sy, sz)) ----

def cube(x0, y0, z0, s):
    return [(x0, y0, z0), (x0 + s, y0, z0), (x0 + s, y0 + s, z0), (x0, y0 + s, z0),
            (x0, y0, z0 + s), (x0 + s, y0, z0 + s), (x0 + s, y0 + s, z0 + s), (x0, y0 + s, z0 + s)]

CUBE_IDX = [0, 3, 2, 1, -1, 4, 5, 6, 7, -1, 0, 1, 5, 4, -1, 2, 3, 7, 6, -1, 1, 2, 6, 5, -1, 0, 4, 7, 3, -1]
offset = lambda idx, off: [i if i == -1 else i + off for i in idx]

# a 2 x 1 quad in z = 0 (area 2) and a triangle in the plane x = 3 (area 0.5)
def ifs_two(solid):
    return ("ifs", [(0, 0, 0), (2, 0, 0), (2, 1, 0), (0, 1, 0), (3, 0, 0), (3, 1, 0), (3, 0, 1)], [0, 1, 2, 3, -1, 4, 5, 6, -1], solid)

CUBE1 = ("ifs", cube(-1, -1, -1, 2), CUBE_IDX, True)
CUBE2 = ("ifs", cube(-3, -1, -1, 1) + cube(2, -1, -1, 1), CUBE_IDX + offset(CUBE_IDX, 8), True)
FLOOR = ("ifs", [(-10, 0, -10), (10, 0, -10), (10, 0, 10), (-10, 0, 10)], [0, 3, 2, 1, -1], False)
CEIL  = ("ifs", [(-10, 4, -10), (10, 4, -10), (10, 4, 10), (-10, 4, 10)], [0, 1, 2, 3, -1], False)
BOX2  = ("box", (2, 2, 2))
BOX4  = ("box", (4, 4, 4))

def g(spec):
    if spec[0] == "box":
        return "Box { size %g %g %g }" % spec[1]
    pts = ", ".join("%g %g %g" % p for p in spec[1])
    return "IndexedFaceSet { solid %s coord Coordinate { point [ %s ] } coordIndex [ %s ] }" % (
        "TRUE" if spec[3] else "FALSE", pts, " ".join(str(i) for i in spec[2]))

def f3(v):
    return "%g %g %g" % tuple(v)

# ---- emitter specs ----
def E(type_, **kw):
    kw["type"] = type_
    return kw

def emitter_text(e):
    t = e["type"]
    out = []
    if e.get("on") is False: out.append("on FALSE")
    if "position" in e: out.append("position " + f3(e["position"]))
    if "direction" in e: out.append("direction " + f3(e["direction"]))
    if "angle" in e: out.append("angle %.9g" % e["angle"])
    if "speed" in e: out.append("speed %g" % e["speed"])
    if "variation" in e: out.append("variation %g" % e["variation"])
    if "mass" in e: out.append("mass %g" % e["mass"])
    if t == "SurfaceEmitter" and "surface" in e: out.append("surface " + g(e["surface"]))
    if t == "VolumeEmitter" and "volume" in e:
        v = e["volume"]
        out.append("coordIndex [ %s ] coord Coordinate { point [ %s ] }" % (" ".join(str(i) for i in v[2]), ", ".join("%g %g %g" % p for p in v[1])))
    return "%s { %s }" % (t, " ".join(out))

def physics_text(p):
    if p[0] == "force": return "ForcePhysicsModel { force %s }" % f3(p[1])
    _, damping, geom, enabled = p
    return "BoundedPhysicsModel { %s%sgeometry %s }" % ("" if enabled else "enabled FALSE ", "" if damping == 1 else "damping %g " % damping, g(geom))

# ---- cases ----
CASES = {}

def case(name, e, n=12, life=1.0, var=0.25, frames=60, phys=(), create=True, dump=None, stat=False, note="", axis=None):
    CASES[name] = dict(E=e, P=list(phys), n=n, life=life, var=var, frames=frames, create=create, dump=dump, stat=stat, note=note, axis=axis)

pt = lambda **kw: E("PointEmitter", **kw)
co = lambda **kw: E("ConeEmitter", **kw)
ex = lambda **kw: E("ExplosionEmitter", **kw)
su = lambda **kw: E("SurfaceEmitter", **kw)
vo = lambda **kw: E("VolumeEmitter", **kw)

# PointEmitter
case("point-dir", pt(position=(1, 2, 3), direction=(0, 1, 0), speed=2, variation=0.25), note="direction * getRandomSpeed ()")
case("point-dirraw", pt(position=(0, 0, 0), direction=(0, 3, 4), speed=5, variation=0.5), note="direction is normalized by X_ITE")
case("point-fixed", pt(position=(-1, 0.5, 2), direction=(1, 0, 0), speed=3, variation=0), var=0, note="no variation: fully seed independent")
case("point-spherical", pt(position=(0, 0, 0), direction=(0, 0, 0), speed=2, variation=0.25), note="zero direction -> getRandomSphericalVelocity ()")
case("point-nospeed", pt(position=(0, 1, 0), direction=(0, 1, 0)), note="speed 0 (default): particles sit at the emitter")
case("point-off", pt(on=False, position=(0, 1, 0), speed=1), note="on FALSE: born hidden (NaN position)")
case("point-create-off", pt(position=(0, 1, 0), speed=1), create=False, note="createParticles FALSE: born hidden")
case("point-spherical-stat", pt(position=(0, 0, 0), direction=(0, 0, 0), speed=2, variation=0.25), n=20000, life=0.5, frames=60, stat=True, axis=(0, 0, 1))

# ConeEmitter
case("cone-basic", co(position=(0, 0, 0), direction=(0, 1, 0), angle=0.5, speed=2, variation=0.25))
case("cone-tilt", co(position=(1, 0, 0), direction=(1, 1, 0), angle=0.3, speed=4, variation=0.1), note="direction not normalized by X_ITE; the quaternion is")
case("cone-zeroangle", co(position=(0, 0, 0), direction=(0, 0, -1), angle=0, speed=2, variation=0), var=0, note="angle 0: direction is exact")
case("cone-wide", co(position=(0, 0, 0), direction=(0, 1, 0), angle=PI, speed=2, variation=0.25), note="angle pi: whole sphere")
case("cone-spherical", co(position=(0, 0, 0), direction=(0, 0, 0), angle=0.5, speed=2, variation=0.25), note="zero direction -> spherical")
case("cone-down", co(position=(0, 0, 0), direction=(0, 0, -1), angle=0.5, speed=2, variation=0.25), note="direction -z is antiparallel to the +z source axis: Quaternion () cross_len == 0 and cos < 0 branch")
case("cone-basic-stat", co(position=(0, 0, 0), direction=(1, 2, -2), angle=0.6, speed=2, variation=0.25), n=20000, life=0.5, frames=60, stat=True, axis=(1 / 3, 2 / 3, -2 / 3))
case("cone-wide-stat", co(position=(0, 0, 0), direction=(0, 1, 0), angle=2.5, speed=2, variation=0.25), n=20000, life=0.5, frames=60, stat=True, axis=(0, 1, 0))

# ExplosionEmitter: the first burst happens when now - creationTime > particleLifetime * (1 + lifetimeVariation)
case("explosion-basic", ex(position=(0, 1, 0), speed=3, variation=0.25), n=16, life=1.0, var=0.25, frames=100)
case("explosion-fixed", ex(position=(0, 0, 0), speed=2, variation=0), n=16, life=1.0, var=0, frames=100, note="no variation: only the directions are random")
case("explosion-stat", ex(position=(0, 0, 0), speed=3, variation=0.25), n=20000, life=1.0, frames=45, stat=True, dump=[38, 39, 45], axis=(0, 0, 1))

# SurfaceEmitter
case("surface-ifs-solid", su(speed=1, variation=0.25, surface=ifs_two(True)), n=16)
case("surface-ifs-twosided", su(speed=1, variation=0.25, surface=ifs_two(False)), n=16, note="solid FALSE: random flip of the normal")
case("surface-box", su(speed=1, variation=0, surface=BOX2), n=16, var=0)
case("surface-nosurface", su(speed=1), n=8, note="no surface: verticesIndex < 0, NaN positions")
case("surface-ifs-twosided-stat", su(speed=1, variation=0.25, surface=ifs_two(False)), n=20000, life=0.5, frames=60, stat=True)
case("surface-box-stat", su(speed=1, variation=0.25, surface=BOX2), n=20000, life=0.5, frames=60, stat=True)

# VolumeEmitter
case("volume-cube", vo(direction=(0, 1, 0), speed=1, variation=0.25, volume=CUBE1), n=16)
case("volume-cube-spherical", vo(direction=(0, 0, 0), speed=1, variation=0.25, volume=CUBE1), n=16)
case("volume-twocubes", vo(direction=(0, 1, 0), speed=1, variation=0, volume=CUBE2), n=16, var=0)
case("volume-empty", vo(speed=1), n=8, note="no coord: whatever the BVH of nothing does")
case("volume-cube-stat", vo(direction=(0, 1, 0), speed=1, variation=0.25, volume=CUBE1), n=20000, life=0.5, frames=60, stat=True)
case("volume-twocubes-stat", vo(direction=(0, 1, 0), speed=1, variation=0.25, volume=CUBE2), n=20000, life=0.5, frames=60, stat=True)

# BoundedPhysicsModel: particles fly down onto a floor at y = 0 (reflect + damping), or spherically inside a box
DOWN = pt(position=(0.3, 2, -0.2), direction=(0, -1, 0), speed=3, variation=0.25)
bm = lambda damping, geom, enabled=True: ("bounded", damping, geom, enabled)
case("bounded-floor", DOWN, n=12, life=2.0, frames=90, phys=[bm(0.5, FLOOR)])
case("bounded-floor-damp1", DOWN, n=12, life=2.0, frames=90, phys=[bm(1, FLOOR)], note="default damping 1")
case("bounded-box", pt(position=(0, 0, 0), direction=(0, 0, 0), speed=3, variation=0.25), n=16, life=3.0, frames=120, phys=[bm(0.9, BOX4)])
case("bounded-two", DOWN, n=12, life=2.0, frames=90, phys=[bm(0.5, FLOOR), bm(0.9, CEIL)],
     note="two models merge into one triangle soup; damping is baked into each model's normals")
case("bounded-disabled", DOWN, n=12, life=2.0, frames=90, phys=[bm(1, FLOOR, False)], note="enabled FALSE: no geometry contributed")
case("bounded-floor-gravity", pt(position=(0.3, 2, -0.2), direction=(0, 0, 0), speed=0, variation=0, mass=1), n=12, life=3.0, var=0, frames=120,
     phys=[("force", (0, -9.8, 0)), bm(0.6, FLOOR)], note="mass 1 + ForcePhysicsModel (context only) + floor")


def world(name, c):
    e = c["E"]
    return (HEAD + "DEF PS ParticleSystem {\n  geometryType \"POINT\"\n  maxParticles %d\n  particleLifetime %g\n  lifetimeVariation %g\n%s  emitter %s\n%s}\n"
            % (c["n"], c["life"], c["var"], "" if c["create"] else "  createParticles FALSE\n", emitter_text(e),
               ("  physics [ %s ]\n" % " ".join(physics_text(p) for p in c["P"])) if c["P"] else ""))


def script(name, c, seed):
    s = {"dt": DT, "t0": 946684800, "frames": c["frames"], "warmup": 8, "seed": seed, "node": "PS"}
    if c["stat"]:
        s["filter"] = "born"
        s["dump"] = c["dump"] or list(range(1, c["frames"] + 1))
    else:
        s["dumpAll"] = True
    return s


if __name__ == "__main__":
    os.makedirs(sdir, exist_ok=True)
    for name, c in sorted(CASES.items()):
        with open(os.path.join(wdir, "particles-%s.x3dv" % name), "w") as f:
            f.write(world(name, c))
        with open(os.path.join(sdir, "particles-%s.json" % name), "w") as f:
            json.dump(script(name, c, zlib.crc32(name.encode()) & 0x7fffffff), f)
    print("%d cases written to %s" % (len(CASES), wdir))
