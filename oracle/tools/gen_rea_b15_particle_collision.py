#!/usr/bin/env python3
"""ParticleSystem avatar collision (lane XITE_R_PARTICLE_AVATAR_COLLISION_02): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b15_particle_collision.py

writes oracle/synthetic-rea-b15-particle-collision/NAME.x3dv and scripts/NAME.json for `x3d-oracle particles` and
x3d-particles-drive. Every script has `collisionProbes`: after each listed frame, the shapes of the frame's COLLISION
traversal, getClosestObject along the six axes and three slanted directions, constrainTranslation of a few moves, the
user pose and positionOffset, and isActive / collideTime of the named Collision nodes. The avatar stands at the origin
looking along -Z with a FLY viewer (no gravity), except in pc-walk-gravity (WALK, falling onto a particle floor). A
FLY viewer is collidable only while it moves (FlyViewer.isCollidable), so those worlds hold an empty enabled Collision,
RUN: with browser .getCollisionCount () > 0 the COLLISION traversal runs every frame. Every
emitter is deterministic (no speed, lifetime or angle variation), so the seeds do not move a particle. The seeded system
is PS. pc-collision-node and pc-geometry have two systems, and X_ITE writes two seeds per frame there while the protocol
records one, so their scripts say `"seedFree": true` (as in batch 14): the Rust run takes no seeds. Newly authored; no
historical content.
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b15-particle-collision"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\n"
LIT = "Appearance { material Material { diffuseColor 1 1 1 } }"


def view(kind="FLY", position="0 0 0", orientation="0 0 1 0"):
    return (f'NavigationInfo {{ type "{kind}" }}\nViewpoint {{ position {position} orientation {orientation} }}\n'
            "Background { skyColor 0.1 0.1 0.2 }\n" + ("DEF RUN Collision { }\n" if kind == "FLY" else ""))


# One particle standing still at `position` (it appears after `lifetime` / maxParticles seconds and is reborn in place).
def still(position):
    return f"PointEmitter {{ position {position} direction 0 1 0 speed 0 variation 0 }}"


# Particles in a row along x in front of the avatar, crossing the -Z collision camera.
ROW = "PointEmitter { position -0.4 0.011 -0.3 direction 1 0 0 speed 0.5 variation 0 }"
# Particles coming at the avatar along +Z and passing through it.
TOWARD = "PointEmitter { position 0.013 0.011 -0.6 direction 0 0 1 speed 1.2 variation 0 }"


def ps(geometry_type, emitter, size="0.2 0.2", lifetime=0.25, max_particles=1, extra="", appearance=LIT, name="PS"):
    return (f'DEF {name} ParticleSystem {{\n  geometryType "{geometry_type}"\n  maxParticles {max_particles}\n'
            f'  particleLifetime {lifetime}\n  lifetimeVariation 0\n  particleSize {size}\n  {extra}\n'
            f'  emitter {emitter}\n  appearance {appearance}\n}}\n')


WALL = ps("QUAD", still("0 0 -0.3"), size="2 2")

DIRECTIONS = [[1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0], [0, -1, 0], [0, 0, -1],
              [0.6, 0, -0.8], [-0.6, 0, -0.8], [0, -0.6, -0.8]]
TRANSLATIONS = [[0, 0, -0.01], [0, 0, -0.1], [0, 0, -0.3], [0.2, 0, -0.2], [-0.05, 0, -0.04], [0, 0, 0.1]]
FRAMES = [5, 15, 30, 45]
EVERY = list(range(1, 46))

CASES = {
    # a QUAD particle as a wall 0.3 in front of the avatar
    "pc-quad-wall": (view() + WALL, {}),
    # the same, disabled at frame 20
    "pc-disabled": (view() + WALL, {"events": [{"frame": 20, "node": "PS", "field": "enabled", "value": False}]}),
    # particles flying through the avatar inside a Collision (isActive, collideTime); a wall to the right inside a
    # disabled Collision is no collision object
    "pc-collision-node": (view() + "DEF COL Collision { children [\n"
                          + ps("QUAD", TOWARD, size="0.3 0.3", lifetime=1.2, max_particles=3) + "] }\n"
                          + "DEF COL_OFF Collision { enabled FALSE children [\n"
                          + ps("QUAD", still("0.2 0 0"), size="2 2", name="PS_OFF") + "] }\n",
                          {"frames": EVERY, "nodes": ["COL", "COL_OFF"], "seedFree": True}),
    # WALK: the avatar falls onto a horizontal QUAD particle floor (terrain following, gravity)
    "pc-walk-gravity": (view("WALK", "0 2.5 0") + "Transform { rotation 1 0 0 -1.5707963 children [\n"
                        + ps("QUAD", still("0 0 0"), size="4 4") + "] }\n", {"frames": EVERY}),
    # long and slanted moves into the wall: the blocked and sliding translations
    "pc-constrain": (view() + WALL, {"translations": TRANSLATIONS + [[0.3, 0, -0.5], [-0.4, 0.1, -0.6], [0, 0, -1],
                                                                     [0.01, 0, -0.06], [0.1, 0.1, -0.051]]}),
    # GEOMETRY particles: a Box and a Sphere
    "pc-geometry": (view() + ps("GEOMETRY", still("0 0 -0.4"), extra="geometry Box { size 0.4 0.4 0.4 }")
                    + ps("GEOMETRY", still("0.35 0 0"), extra="geometry Sphere { radius 0.1 }", name="PS2"), {"seedFree": True}),
    # POINT particles crossing the -Z camera (1 px points of the depth shader)
    "pc-point": (view() + ps("POINT", ROW, lifetime=1.6, max_particles=20), {}),
    # LINE particles crossing it
    "pc-line": (view() + ps("LINE", ROW, size="0.1 0.15", lifetime=1.6, max_particles=20), {}),
    # SPRITE particles turn to the screen under a rotated Transform (a QUAD would turn with it)
    "pc-sprite": (view() + "Transform { translation 0 0 -0.3 rotation 0 1 0 0.9 children [\n"
                  + ps("SPRITE", still("0 0 0"), size="0.6 0.6") + "] }\n", {}),
    # the wall with a bbox far away: addCollisionShape rejects it
    "pc-bbox-culled": (view() + ps("QUAD", still("0 0 -0.3"), size="2 2", extra="bboxCenter 0 0 50 bboxSize 0.1 0.1 0.1"), {}),
    # the wall made invisible at frame 20
    "pc-hidden": (view() + WALL, {"events": [{"frame": 20, "node": "PS", "field": "visible", "value": False}]}),
    # a row of QUAD particles cut to 3 at frame 20
    "pc-max-particles": (view() + ps("QUAD", ROW, size="0.1 0.3", lifetime=1.6, max_particles=20),
                         {"events": [{"frame": 20, "node": "PS", "field": "maxParticles", "value": 3}]}),
    # the wall under a Transform (rotation, non-uniform scale, translation)
    "pc-transform": (view() + "Transform { translation 0.1 0.05 -0.2 rotation 0 1 0 0.4 scale 1.5 0.8 1 children [\n"
                     + ps("QUAD", still("0 0 0"), size="0.5 0.5") + "] }\n", {}),
    # the wall cut by a ClipPlane: only x > 0.1 is left
    "pc-clip-plane": (view() + "Group { children [ ClipPlane { plane 1 0 0 -0.1 }\n" + WALL + "] }\n", {}),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)

    for i, (name, (body, extra)) in enumerate(CASES.items()):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + body)

        probes = {"frames": extra.pop("frames", FRAMES), "directions": DIRECTIONS,
                  "translations": extra.pop("translations", TRANSLATIONS), "nodes": extra.pop("nodes", [])}
        script = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS", "dump": [15, 30, 45],
                  "seed": 15000000 + i, "collisionProbes": probes}
        script.update(extra)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(script, f)

    print(f"{len(CASES)} cases in {ROOT}")


main()
