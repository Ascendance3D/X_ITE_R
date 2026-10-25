#!/usr/bin/env python3
"""ParticleSystem pointing (lane XITE_R_PARTICLE_PICKING_01): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b14_particle_pick.py

writes oracle/synthetic-rea-b14-particle-pick/NAME.x3dv and scripts/NAME.json for `x3d-oracle particles` and
x3d-particles-drive. Every script has `pickProbes`: after each listed frame, browser .touch (x, y) at every point of a
32 px grid over the 512 x 512 canvas (y up), a 5 px one over the middle where the particles are, a 1 px row along
y = 256.5 (the row cases' particles move along it) and a few points off the pixel centers. The seeded system is PS. pp-two-systems has two systems, and X_ITE writes two seeds per frame there while
the protocol records one, so both of its systems are free of random draws that matter (PointEmitter with a direction,
no speed or lifetime variation) and the script says `"seedFree": true`: the Rust run takes no seeds. Only PS is dumped,
so PS2 stands still (its records are exact without a reference) while PS moves through it. Newly authored;
no historical content.
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b14-particle-pick"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\n"
VIEW = "Viewpoint { position 0 0 3 }\nBackground { skyColor 0.1 0.1 0.2 }\n"
LIT = "Appearance { material Material { diffuseColor 1 1 1 } }"

# Particles in a row along x, deterministic: no speed or lifetime variation.
ROW = "PointEmitter { position -0.8 0 0 direction 1 0 0 speed 0.8 variation 0 }"
# Particles coming toward the viewer and spreading: overlapping at different depths, random (seeded).
CONE = "ConeEmitter { position 0 -0.2 -1 direction 0 0.3 1 angle 0.6 speed 0.6 variation 0.3 }"
# The same, slanted: lines and points cross the probe grid.
SLANT = "PointEmitter { position -0.8 -0.4 0 direction 1 0.6 0 speed 0.8 variation 0 }"
STILL = "PointEmitter { direction 0 1 0 speed 0 variation 0 }"


def ps(geometry_type, emitter=CONE, size="0.2 0.2", lifetime=2, max_particles=60, extra="", appearance=LIT, name="PS",
       variation=0.25):
    return (f'DEF {name} ParticleSystem {{\n  geometryType "{geometry_type}"\n  maxParticles {max_particles}\n'
            f'  particleLifetime {lifetime}\n  lifetimeVariation {variation}\n  particleSize {size}\n  {extra}\n'
            f'  emitter {emitter}\n  appearance {appearance}\n}}\n')


def grid(step=32):
    pts = [[x + step / 2, y + step / 2] for y in range(0, 512, step) for x in range(0, 512, step)]
    dense = [[x + 0.5, y + 0.5] for y in range(160, 352, 5) for x in range(96, 416, 5)]
    row = [[x + 0.5, 256.5] for x in range(80, 432)]
    return pts + dense + row + [[256.25, 256.75], [255.9, 256.1], [300.5, 211.5], [128.75, 384.25]]


CASES = {
    # one particle, each geometry type (LINE moves: a still line points along its zero velocity)
    "pp-one-point": (ps("POINT", STILL, lifetime=0.25, max_particles=1, variation=0), {}),
    "pp-one-quad": (ps("QUAD", STILL, size="0.5 0.5", lifetime=0.25, max_particles=1, variation=0), {}),
    "pp-one-geometry": (ps("GEOMETRY", STILL, lifetime=0.25, max_particles=1, variation=0,
                           extra="geometry Box { size 0.4 0.4 0.4 }"), {}),
    # many moving particles of every type
    "pp-row-point": (ps("POINT", ROW, max_particles=20, variation=0), {}),
    "pp-row-point-props": (ps("POINT", ROW, max_particles=20, variation=0,
                              appearance="Appearance { material Material { emissiveColor 1 1 1 } "
                                         "pointProperties PointProperties { pointSizeScaleFactor 8 pointSizeMaxValue 10 } }"), {}),
    "pp-slant-point-props": (ps("POINT", SLANT, max_particles=20, variation=0,
                                appearance="Appearance { material Material { emissiveColor 1 1 1 } "
                                           "pointProperties PointProperties { pointSizeScaleFactor 8 pointSizeMaxValue 10 } }"), {}),
    "pp-row-line": (ps("LINE", ROW, size="0.1 0.15", max_particles=20, variation=0), {}),
    "pp-slant-line": (ps("LINE", SLANT, size="0.1 0.3", max_particles=20, variation=0), {}),
    "pp-cone-triangle": (ps("TRIANGLE"), {}),
    "pp-cone-quad": (ps("QUAD"), {}),
    "pp-cone-sprite": (ps("SPRITE"), {}),
    "pp-cone-geometry": (ps("GEOMETRY", extra="geometry Box { size 0.12 0.12 0.12 }", max_particles=40), {}),
    "pp-cone-geometry-sphere": (ps("GEOMETRY", extra="geometry Sphere { radius 0.08 }", max_particles=40), {}),
    "pp-cone-quad-textured": (ps("QUAD", extra="texCoordKey [0 0.5] texCoord TextureCoordinate { point [0 0, 0.5 0, 0.5 0.5, "
                                               "0 0.5, 0.5 0.5, 1 0.5, 1 1, 0.5 1] }",
                                 appearance="Appearance { texture PixelTexture { image 2 2 4 0xff0000ff 0x00ff00ff "
                                            "0x0000ffff 0xffffffff } }"), {}),
    # under a Transform (rotation, non-uniform scale, translation)
    "pp-transform": ("Transform { translation 0.2 -0.1 0 rotation 0 1 0 0.6 scale 1.5 0.8 1 children [\n"
                     + ps("QUAD") + "] }\n", {}),
    # behind and in front of other geometry
    "pp-behind-box": (ps("QUAD") + 'Transform { translation 0.3 0 0.4 children DEF BOX Shape { appearance ' + LIT
                      + ' geometry Box { size 0.5 0.5 0.1 } } }\n'
                      + 'Transform { translation 0 0 -1.5 children DEF WALL Shape { appearance ' + LIT
                      + ' geometry Box { size 4 4 0.1 } } }\n', {}),
    # two systems, overlapping
    "pp-two-systems": (ps("QUAD", "PointEmitter { position -0.8 0 -0.4 direction 1 0.1 0.5 speed 0.8 variation 0 }",
                          max_particles=20, variation=0)
                       + ps("SPRITE", STILL.replace("PointEmitter {", "PointEmitter { position 0.1 0.05 -0.2"), size="0.5 0.5",
                            lifetime=0.5, max_particles=5, name="PS2", variation=0), {"seedFree": True}),
    # state: disabled at frame 20, invisible, no pointer events, culled by its bbox
    "pp-disable": (ps("QUAD"), {"events": [{"frame": 20, "node": "PS", "field": "enabled", "value": False}]}),
    "pp-invisible": (ps("QUAD"), {"events": [{"frame": 20, "node": "PS", "field": "visible", "value": False}]}),
    "pp-pointer-events": (ps("QUAD"), {"events": [{"frame": 20, "node": "PS", "field": "pointerEvents", "value": False}]}),
    "pp-bbox-culled": (ps("QUAD", extra="bboxCenter 0 0 10 bboxSize 0.1 0.1 0.1"), {}),
    "pp-max-particles": (ps("QUAD"), {"events": [{"frame": 20, "node": "PS", "field": "maxParticles", "value": 5}]}),
    # Overlapping particles at exactly equal depth: a still emitter puts every particle at the same place, so their
    # instance matrices are identical and every covered pixel is a depth tie. The LEQUAL depth test of the pointing
    # pass keeps the one drawn last, so the hit is the highest particle index of the buffer, which is what the
    # instanced draw has to reproduce. (Appended, so the seeds of the cases above do not move.)
    "pp-stack-equal-depth": (ps("QUAD", STILL, size="0.6 0.6", lifetime=2, max_particles=6, variation=0), {}),
    # Overlapping particles at equal depth whose winner is visible: GEOMETRY particles of one flat Rectangle2D, emitted
    # along x at z = 0, so they are coplanar and overlap by two thirds. Every pixel of an overlap is a depth tie, and
    # the hit texCoord says which rectangle the LEQUAL depth test kept — the later one — because the pixel sits at a
    # different place inside each. pp-stack-equal-depth has the ties; this case has the ties and the answer.
    "pp-overlap-equal-depth": (ps("GEOMETRY", ROW, lifetime=2, max_particles=8, variation=0,
                                  extra="geometry Rectangle2D { size 0.6 0.6 }"), {}),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)

    for i, (name, (body, extra)) in enumerate(CASES.items()):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + VIEW + body)

        script = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS", "dump": [15, 30, 45],
                  "seed": 14000000 + i, "pickProbes": {"frames": [15, 30, 45], "points": grid()}}
        script.update(extra)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(script, f)

    print(f"{len(CASES)} cases in {ROOT}")


main()
