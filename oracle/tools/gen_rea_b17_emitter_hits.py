#!/usr/bin/env python3
"""VolumeEmitter intersection limit (lane XITE_R_PARTICLE_EMITTER_HIT_LIMIT_01): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b17_emitter_hits.py

writes oracle/synthetic-rea-b17-emitter-hits/NAME.x3dv and scripts/NAME.json for `x3d-oracle particles` and the
particles_b06_common replay (crates/x3d-viewer/tests/rea_b17_emitter_hits.rs). Every case dumps the whole transform
feedback buffer of PS every frame.

The VolumeEmitter's volume is a stack of N horizontal quads (10 x 10, a few thousandths apart). getRandomPosition
starts a line at a random point of one quad along a random direction around its normal, so nearly every line meets
every quad: N hits (the quad the line starts on included; a steep line near an edge meets fewer). The GLSL
intersection array holds ARRAY_SIZE = 32 entries; the volume getIntersections writes `points [count ++]` without
checking the bound, sorts `count` entries (rounded down to even) and picks a pair among `count / 2`. The particles have
speed 0, so each one stays where it was emitted.

  ve-hits-N           quads listed bottom to top, 0.002 apart (N = 31, 32, 33, 34, 40, 48, 64, 128)
  ve-hits-N-down      the same quads listed top to bottom
  ve-hits-N-shuffle   the same quads in a fixed shuffled order
  ve-hits-N-uneven    uneven spacing (0.001 to 0.006), shuffled
                      (the BVH sorts triangles by centroid; the quads' centroids tie on the split axis, so the listing
                      order sets the traversal order, and with it which hits are stored where)

Issue #93 adds the particles that move (nonzero speed), appended after the cases above so that no seed changes:

  ve-speed-N[-down|-shuffle|-uneven]  the same stacks with direction 0 1 0, speed 2, variation 0.3 and a 1 s lifetime
  ve-speed-64-bounded  64 quads at y = 1.5 emitting downward (speed 2) onto a BoundedPhysicsModel of 40 planes (the floor
                       at y = 0 and 39 planes below it): hits past 32 in the emitter and in the bounded model at once

A BVH deeper than the GLSL stack (BVH_STACK_SIZE = 32) cannot be built: BVH.js splits every node at the median
(leftSize = size >>> 1), so a tree of N triangles has depth ceil(log2 N) and 32 needs more than 2 ** 31 triangles.
That gap has no case; crates/x3d-runtime has a unit test of the depth bound.

Newly authored; no historical content.
"""
import json, os, random

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b17-emitter-hits"))
HEAD = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\n\n"
T0 = 946684800
DT = 1 / 30
HALF = 5


def stack(ys, direction="0 1 0", speed=0, variation=0):
    """VolumeEmitter of horizontal quads at the heights ys, in that order."""
    points = ", ".join(f"{-HALF} {y:.4f} {-HALF}, {HALF} {y:.4f} {-HALF}, {HALF} {y:.4f} {HALF}, {-HALF} {y:.4f} {HALF}"
                       for y in ys)
    index = " ".join(f"{4 * i} {4 * i + 3} {4 * i + 2} {4 * i + 1} -1" for i in range(len(ys)))
    return (f"VolumeEmitter {{ direction {direction} speed {speed:g} variation {variation:g} coordIndex [ {index} ] "
            f"coord Coordinate {{ point [ {points} ] }} }}")


def ps(emitter, n=32, life=0.25, physics=""):
    return (f'DEF PS ParticleSystem {{\n  geometryType "POINT"\n  maxParticles {n}\n  particleLifetime {life:g}\n'
            f"  lifetimeVariation 0\n  emitter {emitter}\n{physics}}}\n")


def up(n):
    return [0.002 * k for k in range(n)]


def shuffled(ys, seed):
    ys = list(ys)
    random.Random(seed).shuffle(ys)
    return ys


def uneven(n, seed):
    rng = random.Random(seed)
    ys, y = [], 0.0

    for _ in range(n):
        ys.append(y)
        y += rng.uniform(0.001, 0.006)

    return shuffled(ys, seed + 1)


CASES = {}

for hits in (31, 32, 33, 34, 40, 48, 64, 128):
    CASES[f"ve-hits-{hits}"] = ps(stack(up(hits)))

for hits in (33, 40, 64, 128):
    CASES[f"ve-hits-{hits}-down"] = ps(stack(up(hits)[::-1]))
    CASES[f"ve-hits-{hits}-shuffle"] = ps(stack(shuffled(up(hits), 1700 + hits)))
    CASES[f"ve-hits-{hits}-uneven"] = ps(stack(uneven(hits, 1800 + hits)))

# Nonzero speed (issue #93). These come after every older case: the script seeds count the cases in order.
MOVING = dict(speed=2, variation=0.3)

for hits in (33, 64, 128):
    CASES[f"ve-speed-{hits}"] = ps(stack(up(hits), **MOVING), life=1)

CASES["ve-speed-64-down"] = ps(stack(up(64)[::-1], **MOVING), life=1)
CASES["ve-speed-64-shuffle"] = ps(stack(shuffled(up(64), 1764), **MOVING), life=1)
CASES["ve-speed-128-uneven"] = ps(stack(uneven(128, 1828), **MOVING), life=1)

FLOOR_PLANES = [0] + [-0.5 - 0.05 * k for k in range(39)]
FLOOR_POINTS = ", ".join(f"-6 {y:g} -6, 6 {y:g} -6, 6 {y:g} 6, -6 {y:g} 6" for y in FLOOR_PLANES)
FLOOR_INDEX = " ".join(f"{4 * i} {4 * i + 3} {4 * i + 2} {4 * i + 1} -1" for i in range(len(FLOOR_PLANES)))
FLOOR = (f"BoundedPhysicsModel {{ damping 0.5 geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point "
         f"[ {FLOOR_POINTS} ] }} coordIndex [ {FLOOR_INDEX} ] }} }}")
CASES["ve-speed-64-bounded"] = ps(stack([1.5 + 0.002 * k for k in range(64)], direction="0 -1 0", speed=2, variation=0.3),
                                  life=3, physics=f"  physics [ {FLOOR} ]\n")


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)

    for i, (name, body) in enumerate(CASES.items()):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + body)

        script = {"dt": DT, "t0": T0, "frames": 40, "warmup": 8, "seed": 17000000 + i, "node": "PS", "dumpAll": True}

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(script, f)

    print(f"{len(CASES)} cases in {ROOT}")


main()
